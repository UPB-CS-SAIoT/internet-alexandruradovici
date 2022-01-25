#include <stdlib.h>
#include "tock.h"
#include "network.h"

static void request_done(int status, int len, int unused, void *ud) {
    bool *done = (bool*)ud;
    *done = true;
}

char * network_get(const char *url, size_t len) {
    char * data_buffer = (char*)malloc (len);
    if (data_buffer != NULL)
    {
        bool done = false;
        allow_ro_return_t ret_allow = allow_readonly(DRIVER_NUM_NETWORK, 0, url, strlen(url));
        if (ret_allow.status == TOCK_STATUSCODE_SUCCESS) {
            allow_rw_return_t ret_allow_buffer = allow_readwrite(DRIVER_NUM_NETWORK, 0, data_buffer, len);
            if (ret_allow_buffer.status == TOCK_STATUSCODE_SUCCESS) {
                subscribe_return_t ret_subscribe = subscribe(DRIVER_NUM_NETWORK, 0, request_done, &done);
                if (ret_subscribe.status == TOCK_STATUSCODE_SUCCESS) 
                {
                    syscall_return_t sys = command(DRIVER_NUM_NETWORK, 1, 0, 0);
                    if (sys.type == TOCK_SYSCALL_SUCCESS)
                    {
                        yield_for(&done);
                    }
                }
            }
            // unallow buffer
            allow_readwrite(DRIVER_NUM_NETWORK, 0, NULL, 0);
        }
        // unallow address
        allow_readonly(DRIVER_NUM_NETWORK, 0, NULL, 0);
    }
    return data_buffer;
}

void network_post(const char *url, const char *payload) {
    allow_ro_return_t ret_allow_address = allow_readonly(DRIVER_NUM_NETWORK, 0, url, strlen(url));
    if (ret_allow_address.status == TOCK_STATUSCODE_SUCCESS) {
        allow_ro_return_t ret_allow_payload = allow_readonly(DRIVER_NUM_NETWORK, 1, payload, strlen(payload));
        if (ret_allow_payload.status == TOCK_STATUSCODE_SUCCESS) {
            syscall_return_t sys = command(DRIVER_NUM_NETWORK, 1, 0, 0);
            if (sys.type == TOCK_SYSCALL_SUCCESS)
            {
                return;
            }
            // unallow payload
            allow_readonly(DRIVER_NUM_NETWORK, 1, NULL, 0);    
        }
        // unallow address
        allow_readonly(DRIVER_NUM_NETWORK, 0, NULL, 0);
    }
}
