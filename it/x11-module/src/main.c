#include <xorg-server.h>
#include <xf86Module.h>
#include <xf86.h>
#include "winit.h"
#include <stdbool.h>
#include <sys/un.h>
#include <unistd.h>
#include <assert.h>

static XF86ModuleVersionInfo winit_version = {
    .modname = "winit",
    .xf86version = XORG_VERSION_CURRENT,
};

enum MessageType {
  MT_NONE,
  MT_CREATE_KEYBOARD,
  MT_CREATE_KEYBOARD_REPLY,
  MT_KEY_PRESS,
  MT_KEY_RELEASE,
  MT_REMOVE_DEVICE,
  MT_ENABLE_SECOND_MONITOR,
  MT_ENABLE_SECOND_MONITOR_REPLY,
  MT_GET_VIDEO_INFO,
  MT_GET_VIDEO_INFO_REPLY,
  MT_CREATE_MOUSE,
  MT_CREATE_MOUSE_REPLY,
  MT_BUTTON_PRESS,
  MT_BUTTON_RELEASE,
  MT_MOUSE_MOVE,
  MT_MOUSE_SCROLL,
  MT_CREATE_TOUCH,
  MT_CREATE_TOUCH_REPLY,
  MT_TOUCH_DOWN,
  MT_TOUCH_DOWN_REPLY,
  MT_TOUCH_UP,
  MT_TOUCH_MOVE,
};

typedef struct {
  uint32_t type;
  uint32_t id;
} CreateKeyboardReply;

typedef struct {
  uint32_t type;
  uint32_t touch_id;
} TouchDownReply;

typedef struct {
  uint32_t type;
  uint32_t second_crtc;
  uint32_t second_output;
  uint32_t first_output;
  uint32_t large_mode_id;
  uint32_t small_mode_id;
} GetVideoInfoReply;

typedef union {
  uint32_t type;
  struct {
    uint32_t type;
    uint32_t id;
    uint32_t key;
  } key_press;
  struct {
    uint32_t type;
    uint32_t id;
    uint32_t touch_id;
  } touch_up;
  struct {
    uint32_t type;
    uint32_t id;
    int32_t x;
    int32_t y;
  } touch_down;
  struct {
    uint32_t type;
    uint32_t id;
    uint32_t touch_id;
    int32_t x;
    int32_t y;
  } touch_move;
  struct {
    uint32_t type;
    uint32_t id;
    int32_t dx;
    int32_t dy;
  } mouse_move;
  struct {
    uint32_t type;
    uint32_t id;
  } remove_device;
  struct {
    uint32_t type;
    uint32_t enable;
  } enable_second_monitor;
} Message;

static void handle_message(int fd, void *closure) {
  Message message;
  assert(read(fd, &message, sizeof(message)) > 0);
  switch (message.type) {
  case MT_CREATE_KEYBOARD: {
    uint32_t id = input_new_keyboard();
    CreateKeyboardReply reply = {
        .type = MT_CREATE_KEYBOARD_REPLY,
        .id = id,
    };
    assert(write(fd, &reply, sizeof(reply)) > 0);
    break;
  }
  case MT_CREATE_MOUSE: {
    uint32_t id = input_new_mouse();
    CreateKeyboardReply reply = {
        .type = MT_CREATE_MOUSE_REPLY,
        .id = id,
    };
    assert(write(fd, &reply, sizeof(reply)) > 0);
    break;
  }
  case MT_CREATE_TOUCH: {
    uint32_t id = input_new_touch();
    CreateKeyboardReply reply = {
        .type = MT_CREATE_TOUCH_REPLY,
        .id = id,
    };
    assert(write(fd, &reply, sizeof(reply)) > 0);
    break;
  }
  case MT_KEY_PRESS:
    input_key_press(message.key_press.id, message.key_press.key);
    break;
  case MT_KEY_RELEASE:
    input_key_release(message.key_press.id, message.key_press.key);
    break;
  case MT_BUTTON_PRESS:
    input_button_press(message.key_press.id, message.key_press.key);
    break;
  case MT_BUTTON_RELEASE:
    input_button_release(message.key_press.id, message.key_press.key);
    break;
  case MT_MOUSE_MOVE:
    input_mouse_move(message.mouse_move.id, message.mouse_move.dx, message.mouse_move.dy);
    break;
  case MT_MOUSE_SCROLL:
    input_mouse_scroll(message.mouse_move.id, message.mouse_move.dx, message.mouse_move.dy);
    break;
  case MT_REMOVE_DEVICE:
    input_remove_device(message.remove_device.id);
    break;
  case MT_ENABLE_SECOND_MONITOR: {
    video_connect_second_monitor(message.enable_second_monitor.enable);
    Message reply = {
        .type = MT_ENABLE_SECOND_MONITOR_REPLY,
    };
    assert(write(fd, &reply, sizeof(reply)) > 0);
    break;
  }
  case MT_GET_VIDEO_INFO: {
    GetVideoInfoReply reply = {
        .type = MT_GET_VIDEO_INFO_REPLY,
    };
    video_get_info(&reply.second_crtc, &reply.first_output, &reply.second_output, &reply.small_mode_id, &reply.large_mode_id);
    assert(write(fd, &reply, sizeof(reply)) > 0);
    break;
  }
  case MT_TOUCH_DOWN: {
    uint32_t touch_id = input_touch_down(message.touch_down.id, message.touch_down.x, message.touch_down.y);
    TouchDownReply reply = {
        .type = MT_TOUCH_DOWN_REPLY,
        .touch_id = touch_id,
    };
    assert(write(fd, &reply, sizeof(reply)) > 0);
    break;
  }
  case MT_TOUCH_MOVE: {
    input_touch_move(message.touch_move.id, message.touch_move.touch_id, message.touch_move.x, message.touch_move.y);
    break;
  }
  case MT_TOUCH_UP: {
    input_touch_up(message.touch_move.id, message.touch_move.touch_id);
    break;
  }
  default:
    LogMessage(X_ERROR, "Invalid message type %u\n", message.type);
    assert(0 && "Invalid message type");
  }
}

static pointer winit_setup(pointer module, pointer opts, int *errmaj,
                           int *errmin) {
  static bool done = false;

  if (done) {
    if (errmaj) {
      *errmaj = LDR_ONCEONLY;
    }
    return NULL;
  }
  done = true;

  video_init(module);
  input_init(module);

  char *socknum = getenv("WINIT_IT_SOCKET");
  assert(socknum);
  int sock = atoi(socknum);
  xf86AddGeneralHandler(sock, handle_message, NULL);

  return (void *)1;
}

__attribute__((visibility("default"))) XF86ModuleData winitModuleData = {
    .vers = &winit_version,
    .setup = winit_setup,
};
