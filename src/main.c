#include "stdio.h"
#include "stdlib.h"
#include "signal.h"
#include "string.h"
#include "portmidi.h"
#include "porttime.h"
#include "unistd.h"

#define BUFFER_SIZE 1024

typedef struct MidiDevice {
  int id;
  const PmDeviceInfo* info;
  PortMidiStream* stream;
} MidiDevice;

typedef struct MidiDevices {
  int inputLength;
  MidiDevice* inputDevices;
  int outputLength;
  MidiDevice* outputDevices;
} MidiDevices;

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

void pollEvents(PtTimestamp timestamp, void* data) {
  MidiDevices* devices = (MidiDevices*) data;
  PmEvent buffer[BUFFER_SIZE];
  PmError error;

  for (int i = 0; i < devices->inputLength; i++) {
    error = Pm_Read(devices->inputDevices[i].stream, buffer, BUFFER_SIZE);
    if (error < 0) {
      printf("%s\n", Pm_GetErrorText(error));
    } else if (error > 0) {
      for (int j = 0; j < devices->outputLength; j++) {
        Pm_Write(devices->outputDevices[j].stream, buffer, error);
      }
    }
  }
}

int main(int argc, const char **argv) {
  int devicesCount = Pm_CountDevices();
  const PmDeviceInfo* info = NULL;

  MidiDevices devices = { 0, NULL, 0, NULL };

  catchSignals();

  Pt_Start(10, &pollEvents, &devices);

  for (int i = 0; i < devicesCount; i++) {
    info = Pm_GetDeviceInfo(i);

    if (info->input) {
      devices.inputLength++;

      devices.inputDevices = realloc(devices.inputDevices, sizeof(MidiDevice) * devices.inputLength);
      devices.inputDevices[devices.inputLength - 1].id = i;
      devices.inputDevices[devices.inputLength - 1].info = info;
      devices.inputDevices[devices.inputLength - 1].stream = NULL;

      Pm_OpenInput(&devices.inputDevices[devices.inputLength - 1].stream, i, NULL, BUFFER_SIZE, NULL, NULL);
      printf("Found input: %s\n", info->name);
    } else if (info->output) {
      devices.outputLength++;

      devices.outputDevices = realloc(devices.outputDevices, sizeof(MidiDevice) * devices.outputLength);
      devices.outputDevices[devices.outputLength - 1].id = i;
      devices.outputDevices[devices.outputLength - 1].info = info;
      devices.outputDevices[devices.outputLength - 1].stream = NULL;

      Pm_OpenOutput(&devices.outputDevices[devices.outputLength - 1].stream, i, NULL, BUFFER_SIZE, NULL, NULL, 0);
      printf("Found output: %s\n", info->name);
    }
  }

  while (!done) {
    sleep(1);
  }

  free(devices.inputDevices);
  free(devices.outputDevices);

  return 0;
}
