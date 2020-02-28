#include "stdio.h"
#include "stdlib.h"
#include "signal.h"
#include "string.h"
#include "portmidi.h"
#include "porttime.h"
#include "unistd.h"

#define BUFFER_SIZE 1024

typedef struct MidiInputDevice {
  int id;
  const PmDeviceInfo* info;
  PortMidiStream* stream;
} MidiInputDevice;

typedef struct MidiInputDevices {
  int length;
  MidiInputDevice* devices;
} MidiInputDevices;

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
  MidiInputDevices* inputDevices = (MidiInputDevices*) data;
  PmEvent buffer[BUFFER_SIZE];
  PmError error;

  for (int i = 0; i < inputDevices->length; i++) {
    error = Pm_Read(inputDevices->devices[i].stream, buffer, BUFFER_SIZE);
    if (error < 0) {
      printf("%s\n", Pm_GetErrorText(error));
    } else if (error > 0) {
      for (int j = 0; j < error; j++) {
        printf("Event(%d,%d,%d)\n", Pm_MessageStatus(buffer[j].message), Pm_MessageData1(buffer[j].message), Pm_MessageData2(buffer[j].message));
      }
    }
  }
}

int main(int argc, const char **argv) {
  int devicesCount = Pm_CountDevices();
  const PmDeviceInfo* info = NULL;

  MidiInputDevices inputDevices = { 0, NULL };

  catchSignals();

  Pt_Start(10, &pollEvents, &inputDevices);

  for (int i = 0; i < devicesCount; i++) {
    info = Pm_GetDeviceInfo(i);

    if (info->input) {
      inputDevices.length++;

      inputDevices.devices = realloc(inputDevices.devices, sizeof(MidiInputDevice) * inputDevices.length);
      inputDevices.devices[inputDevices.length - 1].id = i;
      inputDevices.devices[inputDevices.length - 1].info = info;
      inputDevices.devices[inputDevices.length - 1].stream = NULL;

      Pm_OpenInput(&inputDevices.devices[inputDevices.length - 1].stream, i, NULL, BUFFER_SIZE, NULL, NULL);
      printf("Found input: %s\n", info->name);
    }
  }

  while (!done) {
    sleep(1);
  }

  free(inputDevices.devices);

  return 0;
}
