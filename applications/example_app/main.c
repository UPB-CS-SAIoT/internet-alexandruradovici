/* vim: set sw=2 expandtab tw=80: */

#include <stdio.h>
#include <stdlib.h>
#include "tock.h"
#include "network.h"

int main(void) {
  if (driver_exists(DRIVER_NUM_NETWORK)) {
    char* data = network_get("http://www.google.com", 1024);
    if (data != NULL)
    {
      printf ("Got: %s\n", data);
      free(data);
    }
    else
    {
      printf("Out of memory\n");
    }
  }
  else
  {
    printf("No network driver\n");
  }
  return 0;
}
