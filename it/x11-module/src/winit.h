#pragma once

#include <X11/Xdefs.h>
#include <stdint.h>

void video_init(pointer module);
void video_connect_second_monitor(uint32_t connected);
void video_get_info(uint32_t *second_crtc, uint32_t *first_output, uint32_t *second_output, uint32_t *small_mode_id, uint32_t *large_mode_id);

void input_init(pointer module);

uint32_t input_new_keyboard();
void input_key_press(uint32_t keyboard, uint8_t key);
void input_key_release(uint32_t keyboard, uint8_t key);

uint32_t input_new_mouse();
void input_button_press(uint32_t mouse, uint8_t button);
void input_button_release(uint32_t mouse, uint8_t button);
void input_mouse_move(uint32_t mouse, int32_t dx, int32_t dy);
void input_mouse_scroll(uint32_t mouse, int32_t dx, int32_t dy);

uint32_t input_new_touch();
uint32_t input_touch_down(uint32_t touch, int32_t x, int32_t y);
void input_touch_up(uint32_t touch, uint32_t touch_id);
void input_touch_move(uint32_t touch, uint32_t touch_id, int32_t x, int32_t y);

void input_remove_device(uint32_t id);
