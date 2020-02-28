#include "stdio.h"
#include "stdlib.h"
#include "signal.h"
#include "string.h"
#include "portmidi.h"
#include "unistd.h"

volatile sig_atomic_t done = 0;

void terminate(int signum) {
  done = 1;
}

void catchSignals() {
  struct sigaction action;
  memset(&action, 0, sizeof(struct sigaction));
  action.sa_handler = terminate;
  sigaction(SIGTERM, &action, NULL);
  sigaction(SIGINT, &action, NULL);
}

int main(int argc, const char **argv) {
  int devicesCount = Pm_CountDevices();
  const PmDeviceInfo* info = NULL;

  int inputDevicesCount = 0;
  PmDeviceInfo* inputDevices = NULL;

  int outputDevicesCount = 0;
  PmDeviceInfo* outputDevices = NULL;

  catchSignals();

  for (int i = 0; i < devicesCount; i++) {
    info = Pm_GetDeviceInfo(i);

    if (info->input) {
      inputDevicesCount++;
      inputDevices = realloc(inputDevices, sizeof(PmDeviceInfo) * inputDevicesCount);
      inputDevices[inputDevicesCount-1] = *info;
      printf("Found input: %s\n", info->name);
    } else {
      outputDevicesCount++;
      outputDevices = realloc(outputDevices, sizeof(PmDeviceInfo) * outputDevicesCount);
      outputDevices[outputDevicesCount-1] = *info;
      printf("Found output: %s\n", info->name);
    }
  }

  while (!done) {
    sleep(1);
  }

  free(inputDevices);
  free(outputDevices);

  return 0;
}
