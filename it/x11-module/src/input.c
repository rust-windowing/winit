#include <xorg-server.h>
#include <X11/Xdefs.h>
#include <xf86Xinput.h>
#include <stdint.h>
#include <exevents.h>
#include <xserver-properties.h>
#include "winit.h"

#define DRIVER_VERSION 1
#define DRIVER_NAME "winit_input"

typedef enum {
  TyKeyboard = 1,
  TyMouse,
  TyTouch,
} Type;

typedef struct Device {
  struct Device *next;
  struct Device **prev_next;
  Type type;
  InputInfoPtr device;
  ValuatorMask *mask;
} Device;

static Device *devices;
static Type current_type;

static void ptr_control(DeviceIntPtr dev, PtrCtrl *ctrl) { }

static void init_pointer(DeviceIntPtr dev, Device *device) {
  Atom button_labels[] = {
      XIGetKnownProperty(BTN_LABEL_PROP_BTN_LEFT),
      XIGetKnownProperty(BTN_LABEL_PROP_BTN_RIGHT),
      XIGetKnownProperty(BTN_LABEL_PROP_BTN_MIDDLE),
      XIGetKnownProperty(BTN_LABEL_PROP_BTN_WHEEL_UP),
      XIGetKnownProperty(BTN_LABEL_PROP_BTN_WHEEL_DOWN),
      XIGetKnownProperty(BTN_LABEL_PROP_BTN_HWHEEL_LEFT),
      XIGetKnownProperty(BTN_LABEL_PROP_BTN_HWHEEL_RIGHT),
      XIGetKnownProperty(BTN_LABEL_PROP_BTN_SIDE),
      XIGetKnownProperty(BTN_LABEL_PROP_BTN_EXTRA),
  };
  Atom valuator_labels[] = {
      XIGetKnownProperty(AXIS_LABEL_PROP_REL_X),
      XIGetKnownProperty(AXIS_LABEL_PROP_REL_Y),
      XIGetKnownProperty(AXIS_LABEL_PROP_REL_HWHEEL),
      XIGetKnownProperty(AXIS_LABEL_PROP_REL_WHEEL),
  };
  uint8_t button_map[] = { 0, 1, 2, 3, 4, 5, 6, 7, 8, 9 };
  assert(InitPointerDeviceStruct(&dev->public, button_map, 9, button_labels, ptr_control, GetMotionHistorySize(), 4, valuator_labels));
  device->mask = valuator_mask_new(4);
  assert(device->mask);
}

static void init_touch(DeviceIntPtr dev, Device *device) {
  init_pointer(dev, device);
  xf86InitValuatorAxisStruct(dev, 0, XIGetKnownProperty(AXIS_LABEL_PROP_ABS_MT_POSITION_X), 0, 1023, 0, 0, 0, Absolute);
  xf86InitValuatorAxisStruct(dev, 1, XIGetKnownProperty(AXIS_LABEL_PROP_ABS_MT_POSITION_Y), 0, 767, 0, 0, 0, Absolute);
  assert(InitTouchClassDeviceStruct(dev, 2, XIDirectTouch, 2));
  assert(InitPointerAccelerationScheme(dev, PtrAccelNoOp));
}

static void init_mouse(DeviceIntPtr dev, Device *device) {
  init_pointer(dev, device);
  xf86InitValuatorAxisStruct(dev, 0, XIGetKnownProperty(AXIS_LABEL_PROP_REL_X), -1, -1, 0, 0, 0, Relative);
  xf86InitValuatorAxisStruct(dev, 1, XIGetKnownProperty(AXIS_LABEL_PROP_REL_Y), -1, -1, 0, 0, 0, Relative);
  SetScrollValuator(dev, 2, SCROLL_TYPE_HORIZONTAL, 120, 0);
  SetScrollValuator(dev, 3, SCROLL_TYPE_VERTICAL, 120, 0);
  assert(InitPointerAccelerationScheme(dev, PtrAccelNoOp));
}

static int device_control(DeviceIntPtr dev, int what) {
  InputInfoPtr pInfo = dev->public.devicePrivate;
  Device *device = pInfo->private;

  switch (what) {
  case DEVICE_INIT:
    switch (device->type) {
    case TyKeyboard:
      assert(InitKeyboardDeviceStruct(dev, NULL, NULL, NULL));
      break;
    case TyMouse:
      init_mouse(dev, device);
      break;
    case TyTouch:
      init_touch(dev, device);
      break;
    }
  case DEVICE_ON:
  case DEVICE_OFF:
  case DEVICE_CLOSE:
    return Success;
  default:
    return BadValue;
  }
}

static int pre_init(InputDriverPtr drv, InputInfoPtr pInfo, int flags) {
  Device *device = calloc(sizeof(*device), 1);
  device->device = pInfo;
  device->type = current_type;
  pInfo->private = device;
  switch (current_type) {
  case TyKeyboard:
    pInfo->type_name = XI_KEYBOARD;
    break;
  case TyMouse:
    pInfo->type_name = XI_MOUSE;
    break;
  case TyTouch:
    pInfo->type_name = XI_TOUCHSCREEN;
    break;
  default:
    assert(0 && "Invalid type");
  }
  pInfo->device_control = device_control;

  device->prev_next = &devices;
  while (*device->prev_next) {
    device->prev_next = &(*device->prev_next)->next;
  }
  *device->prev_next = device;

  return Success;
}

static void un_init(InputDriverPtr drv, InputInfoPtr pInfo, int flags) {
  Device *device = pInfo->private;
  pInfo->private = NULL;
  *device->prev_next = device->next;
  if (device->next) {
    device->next->prev_next = device->prev_next;
  }
  if (device->mask) {
    valuator_mask_free(&device->mask);
  }
  free(device);
}

void input_init(pointer module) {
  static InputDriverRec driver = {
      .driverVersion = DRIVER_VERSION,
      .driverName = DRIVER_NAME,
      .PreInit = pre_init,
      .UnInit = un_init,
  };
  xf86AddInputDriver(&driver, module, 0);
}

static uint32_t input_new(const char *prefix) {
  static int next_input_id = 1;

  InputOption *options = NULL;
  char *name;
  uint32_t id = next_input_id++;
  asprintf(&name, "%s%u", prefix, id);
  options = input_option_new(options, "driver", strdup(DRIVER_NAME));
  options = input_option_new(options, "name", name);
  options = input_option_new(options, "floating", strdup("1"));
  DeviceIntPtr dev;
  assert(!NewInputDeviceRequest(options, NULL, &dev));
  input_option_free_list(&options);
  return (uint32_t)dev->id;
}

uint32_t input_new_keyboard() {
  current_type = TyKeyboard;
  return input_new("keyboard");
}

uint32_t input_new_mouse() {
  current_type = TyMouse;
  return input_new("mouse");
}

uint32_t input_new_touch() {
  current_type = TyTouch;
  return input_new("touchscreen");
}

#define MIN_KEYCODE 8

static Device *get_device(uint32_t id) {
  Device *device = devices;
  while (device) {
    if (device->device->dev->id == id) {
      break;
    }
    device = device->next;
  }
  assert(device);
  return device;
}

static Device *get_keyboard(uint32_t keyboard) {
  Device *device = get_device(keyboard);
  assert(device->type == TyKeyboard);
  return device;
}

static Device *get_mouse(uint32_t mouse) {
  Device *device = get_device(mouse);
  assert(device->type == TyMouse);
  return device;
}

static Device *get_touch(uint32_t touch) {
  Device *device = get_device(touch);
  assert(device->type == TyTouch);
  return device;
}

void input_key_press(uint32_t keyboard, uint8_t key) {
  Device *device = get_keyboard(keyboard);
  xf86PostKeyboardEvent(device->device->dev, key + MIN_KEYCODE, 1);
}

void input_key_release(uint32_t keyboard, uint8_t key) {
  Device *device = get_keyboard(keyboard);
  xf86PostKeyboardEvent(device->device->dev, key + MIN_KEYCODE, 0);
}

void input_button_press(uint32_t mouse, uint8_t button) {
  Device *device = get_mouse(mouse);
  xf86PostButtonEvent(device->device->dev, Relative, button, 1, 0, 0);
}

void input_button_release(uint32_t mouse, uint8_t button) {
  Device *device = get_mouse(mouse);
  xf86PostButtonEvent(device->device->dev, Relative, button, 0, 0, 0);
}

void input_mouse_move(uint32_t mouse, int32_t dx, int32_t dy) {
  Device *device = get_mouse(mouse);
  valuator_mask_zero(device->mask);
  ErrorF("%d %d\n", dx, dy);
  valuator_mask_set_unaccelerated(device->mask, 0, dx, dx);
  valuator_mask_set_unaccelerated(device->mask, 1, dy, dy);
  xf86PostMotionEventM(device->device->dev, Relative, device->mask);
}

void input_mouse_scroll(uint32_t mouse, int32_t dx, int32_t dy) {
  Device *device = get_mouse(mouse);
  valuator_mask_zero(device->mask);
  if (dx) {
    valuator_mask_set(device->mask, 2, dx * 120);
  }
  if (dy) {
    valuator_mask_set(device->mask, 3, dy * 120);
  }
  xf86PostMotionEventM(device->device->dev, Relative, device->mask);
}

uint32_t input_touch_down(uint32_t touch, int32_t x, int32_t y) {
  static uint32_t TOUCH_ID = 1;
  Device *device = get_touch(touch);
  valuator_mask_zero(device->mask);
  valuator_mask_set(device->mask, 0, x);
  valuator_mask_set(device->mask, 1, y);
  uint32_t touch_id = TOUCH_ID++;
  xf86PostTouchEvent(device->device->dev, touch_id, XI_TouchBegin, 0, device->mask);
  return touch_id;
}

void input_touch_up(uint32_t touch, uint32_t touch_id) {
  Device *device = get_touch(touch);
  xf86PostTouchEvent(device->device->dev, touch_id, XI_TouchEnd, 0, 0);
}

void input_touch_move(uint32_t touch, uint32_t touch_id, int32_t x, int32_t y) {
  Device *device = get_touch(touch);
  valuator_mask_zero(device->mask);
  valuator_mask_set(device->mask, 0, x);
  valuator_mask_set(device->mask, 1, y);
  xf86PostTouchEvent(device->device->dev, touch_id, XI_TouchUpdate, 0, device->mask);
}

void input_remove_device(uint32_t id) {
  Device *device = get_device(id);
  DeleteInputDeviceRequest(device->device->dev);
}
