//! Network Driver
//! Commands
//!     0 -> SUCCESS
//!     1 -> Network buffer
//!     2 -> characters Networked
//!
//! Allow
//!     0 -> buffer to display
//!

// GET/POST address data_out (base64)\n
use core::cell::Cell;

use kernel::errorcode::into_statuscode;
use kernel::grant::Grant;
use kernel::hil::uart::{ReceiveClient, TransmitClient, UartData};
use kernel::process::{Error, ProcessId};
use kernel::processbuffer::{
    ReadOnlyProcessBuffer, ReadWriteProcessBuffer, ReadableProcessBuffer, WriteableProcessBuffer,
};
use kernel::syscall::{CommandReturn, SyscallDriver};
use kernel::utilities::cells::TakeCell;
use kernel::{debug, ErrorCode};

pub const DRIVER_NUM: usize = 0xa0001;

#[derive(Copy, Clone)]
enum NetworkState {
    Idle,
    Requesting(ProcessId),
}

#[derive(Default)]
pub struct AppStorage {
    address: ReadOnlyProcessBuffer,
    data_out: ReadOnlyProcessBuffer,
    data_in: ReadWriteProcessBuffer,
}

pub struct Network<'a> {
    grant_access: Grant<AppStorage, 1>,
    uart: &'a dyn UartData<'a>,
    state: Cell<NetworkState>,
    buffer: TakeCell<'static, [u8]>,
}

impl<'a> Network<'a> {
    pub fn new(
        grant_access: Grant<AppStorage, 1>,
        uart: &'a dyn UartData<'a>,
        buffer: &'static mut [u8],
    ) -> Network<'a> {
        Network {
            grant_access,
            uart: uart,
            state: Cell::new(NetworkState::Idle),
            buffer: TakeCell::new(buffer),
        }
    }
}

impl<'a> SyscallDriver for Network<'a> {
    fn command(
        &self,
        command_num: usize,
        _r2: usize,
        _r3: usize,
        process_id: ProcessId,
    ) -> CommandReturn {
        match command_num {
            0 => CommandReturn::success(),
            // send request
            1 => {
                if let NetworkState::Idle = self.state.get() {
                    let res = self
                        .grant_access
                        .enter(process_id, |app_storage, _upcalls_table| {
                            // Result<Result<(), ErrorCode>, Error>
                            let res = app_storage.address.enter(|address| {
                                let buffer = self.buffer.take();
                                if let Some(buffer) = buffer {
                                    // buf[index].get() -> u8
                                    if 5 + address.len() <= buffer.len() {
                                        address.copy_to_slice(&mut buffer[5..address.len() + 5]);
                                        buffer[5 + address.len()] = ' ' as u8;
                                        if app_storage.data_out.len() > 0 {
                                            // POST
                                            app_storage
                                                .data_out
                                                .enter(move |data_out| {
                                                    if 5 + address.len() + data_out.len()
                                                        <= buffer.len()
                                                    {
                                                        data_out.copy_to_slice(
                                                            &mut buffer[5 + address.len() + 1
                                                                ..5 + address.len()
                                                                    + 1
                                                                    + data_out.len()],
                                                        );
                                                        &buffer[0..5]
                                                            .copy_from_slice("POST ".as_bytes());
                                                        buffer[5
                                                            + address.len()
                                                            + 1
                                                            + data_out.len()] = '\n' as u8;
                                                        if let Err((error, buffer)) =
                                                            self.uart.transmit_buffer(
                                                                buffer,
                                                                5 + address.len()
                                                                    + 1
                                                                    + data_out.len()
                                                                    + 1,
                                                            )
                                                        {
                                                            self.buffer.replace(buffer);
                                                            Err(error)
                                                        } else {
                                                            self.state.set(
                                                                NetworkState::Requesting(
                                                                    process_id,
                                                                ),
                                                            );
                                                            Ok(())
                                                        }
                                                    } else {
                                                        Err(ErrorCode::INVAL)
                                                    }
                                                })
                                                .map_err(|err| err.into())
                                                .and_then(|x| x)
                                        } else {
                                            // GET
                                            &buffer[0..5].copy_from_slice("GET  ".as_bytes());
                                            buffer[5 + address.len()] = '\n' as u8;
                                            if let Err((error, buffer)) = self
                                                .uart
                                                .transmit_buffer(buffer, 5 + address.len() + 1)
                                            {
                                                self.buffer.replace(buffer);
                                                Err(error)
                                            } else {
                                                self.state
                                                    .set(NetworkState::Requesting(process_id));
                                                Ok(())
                                            }
                                        }
                                    } else {
                                        Err(ErrorCode::SIZE)
                                    }
                                } else {
                                    Err(ErrorCode::NOMEM)
                                }
                            });
                            match res {
                                Ok(Ok(())) => Ok(()),
                                Ok(Err(err)) => Err(err),
                                Err(err) => Err(err.into()),
                            }
                        });
                    match res {
                        Ok(Ok(())) => CommandReturn::success(),
                        Ok(Err(err)) => CommandReturn::failure(err),
                        Err(err) => CommandReturn::failure(err.into()),
                    }
                } else {
                    CommandReturn::failure(ErrorCode::BUSY)
                }
            }
            // 2 => {
            //     let res = self
            //         .grant_access
            //         .enter(process_id, |app_storage, _| app_storage.counter);
            //     match res {
            //         Ok(counter) => CommandReturn::success_u32(counter),
            //         Err(err) => CommandReturn::failure(err.into()),
            //     }
            // }
            _ => CommandReturn::failure(ErrorCode::NOSUPPORT),
        }
    }

    fn allow_readonly(
        &self,
        process_id: ProcessId,
        allow_num: usize,
        mut buffer: ReadOnlyProcessBuffer,
    ) -> Result<ReadOnlyProcessBuffer, (ReadOnlyProcessBuffer, ErrorCode)> {
        match allow_num {
            // address
            0 => {
                let res = self
                    .grant_access
                    .enter(process_id, |app_storage, _upcalls_table| {
                        core::mem::swap(&mut app_storage.address, &mut buffer);
                    });
                match res {
                    Ok(()) => Ok(buffer),
                    Err(err) => Err((buffer, err.into())),
                }
            }
            // data_out
            1 => {
                let res = self
                    .grant_access
                    .enter(process_id, |app_storage, _upcalls_table| {
                        core::mem::swap(&mut app_storage.data_out, &mut buffer);
                    });
                match res {
                    Ok(()) => Ok(buffer),
                    Err(err) => Err((buffer, err.into())),
                }
            }
            _ => Err((buffer, ErrorCode::NOSUPPORT)),
        }
    }

    fn allow_readwrite(
        &self,
        process_id: ProcessId,
        allow_num: usize,
        mut buffer: ReadWriteProcessBuffer,
    ) -> Result<ReadWriteProcessBuffer, (ReadWriteProcessBuffer, ErrorCode)> {
        match allow_num {
            // data_in
            0 => {
                let res = self
                    .grant_access
                    .enter(process_id, |app_storage, _upcalls_table| {
                        core::mem::swap(&mut app_storage.data_in, &mut buffer);
                    });
                match res {
                    Ok(()) => Ok(buffer),
                    Err(err) => Err((buffer, err.into())),
                }
            }
            _ => Err((buffer, ErrorCode::NOSUPPORT)),
        }
    }

    fn allocate_grant(&self, process_id: ProcessId) -> Result<(), Error> {
        self.grant_access
            .enter(process_id, |_app_storage, _upcalls_table| {})
    }
}

impl<'a> TransmitClient for Network<'a> {
    fn transmitted_buffer(
        &self,
        tx_buffer: &'static mut [u8],
        _tx_len: usize,
        rval: Result<(), ErrorCode>,
    ) {
        match rval {
            Ok(()) => {
                if let Err((error, buffer)) = self.uart.receive_buffer(tx_buffer, 1) {
                    self.buffer.replace(buffer);
                    if let NetworkState::Requesting(process_id) = self.state.get() {
                        let _ = self.grant_access.enter(process_id, |_, upcalls_table| {
                            let _ = upcalls_table
                                .schedule_upcall(0, (into_statuscode(Err(error)), 0, 0));
                        });
                    }
                    self.state.set(NetworkState::Idle);
                }
            }
            Err(error) => {
                self.buffer.replace(tx_buffer);
                if let NetworkState::Requesting(process_id) = self.state.get() {
                    let _ = self.grant_access.enter(process_id, |_, upcalls_table| {
                        let _ =
                            upcalls_table.schedule_upcall(0, (into_statuscode(Err(error)), 0, 0));
                    });
                }
                self.state.set(NetworkState::Idle);
            }
        }
    }
}

impl<'a> ReceiveClient for Network<'a> {
    fn received_buffer(
        &self,
        rx_buffer: &'static mut [u8],
        rx_len: usize,
        rval: Result<(), ErrorCode>,
        _error: kernel::hil::uart::Error,
    ) {
        match rval {
            Ok(()) => {
                debug!("received data");
                if let NetworkState::Requesting(process_id) = self.state.get() {
                    let _ = self
                        .grant_access
                        .enter(process_id, |app_storage, upcalls_table| {
                            let res = app_storage
                                .data_in
                                .mut_enter(|data_in| {
                                    if rx_buffer.len() < data_in.len() {
                                        data_in.copy_from_slice(&rx_buffer);
                                        data_in[rx_len - 1].set(0);
                                    }
                                })
                                .map_err(|err| err.into());
                            let _ =
                                upcalls_table.schedule_upcall(0, (into_statuscode(res), rx_len, 0));
                        });
                }
            }
            Err(error) => {
                if let NetworkState::Requesting(process_id) = self.state.get() {
                    let _ = self.grant_access.enter(process_id, |_, upcalls_table| {
                        let _ =
                            upcalls_table.schedule_upcall(0, (into_statuscode(Err(error)), 0, 0));
                    });
                }
            }
        }
        self.buffer.replace(rx_buffer);
        self.state.set(NetworkState::Idle);
    }
}
