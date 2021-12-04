#include <xorg-server.h>
#include "winit.h"
#include <fb.h>
#include <micmap.h>
#include <mipointer.h>
#include <property.h>
#include <xf86.h>
#include <xf86Crtc.h>
#include <xf86cmap.h>
#include <xf86fbman.h>
#include <X11/X.h>
#include <xf86RandR12.h>
#include <stdbool.h>

#define DRIVER_VERSION 1
#define DRIVER_NAME "winit"

#define WIDTH (1024 * 2)
#define HEIGHT 768

#define NUM_OUTPUTS 2

typedef struct {
  bool connected;
  xf86CrtcPtr crtc;
  xf86OutputPtr output;
} DriverOutput;

static struct {
  ScreenPtr screen;
  DriverOutput outputs[NUM_OUTPUTS];
  char pixels[WIDTH * HEIGHT * 4];
} driver;

static Bool switch_mode(ScrnInfoPtr arg, DisplayModePtr mode) { return TRUE; }

static Bool save_screen(ScreenPtr pScreen, int mode) { return TRUE; }

static ModeStatus valid_mode(ScrnInfoPtr arg, DisplayModePtr mode, Bool verbose,
                             int flags) {
  return MODE_OK;
}

static void crtc_dpms(xf86CrtcPtr crtc, int mode) {}

static Bool crtc_lock(xf86CrtcPtr crtc) { return FALSE; }

static Bool crtc_mode_fixup(xf86CrtcPtr crtc, DisplayModePtr mode,
                            DisplayModePtr adjusted_mode) {
  return TRUE;
}

static void crtc_stub(xf86CrtcPtr crtc) {}

static void crtc_gamma_set(xf86CrtcPtr crtc, CARD16 *red, CARD16 *green,
                           CARD16 *blue, int size) {}

static void *crtc_shadow_allocate(xf86CrtcPtr crtc, int width, int height) {
  return NULL;
}

static void crtc_mode_set(xf86CrtcPtr crtc, DisplayModePtr mode,
                          DisplayModePtr adjusted_mode, int x, int y) {}

static void output_stub(xf86OutputPtr output) {}

static void output_dpms(xf86OutputPtr output, int mode) {}

static int output_mode_valid(xf86OutputPtr output, DisplayModePtr mode) {
  return MODE_OK;
}

static Bool output_mode_fixup(xf86OutputPtr output, DisplayModePtr mode,
                              DisplayModePtr adjusted_mode) {
  return TRUE;
}

static void output_mode_set(xf86OutputPtr output, DisplayModePtr mode,
                            DisplayModePtr adjusted_mode) {}

static Bool enter_vt(ScrnInfoPtr arg) { return TRUE; }

static void leave_vt(ScrnInfoPtr arg) {}

static void load_palette(ScrnInfoPtr pScrn, int numColors, int *indices,
                         LOCO *colors, VisualPtr pVisual) {}

static Bool crtc_config_resize(ScrnInfoPtr pScrn, int width, int height) {
  if (width <= WIDTH && height <= HEIGHT) {
    pScrn->virtualX = width;
    pScrn->virtualY = height;
    pScrn->displayWidth = width;
    return TRUE;
  }
  return FALSE;
}

static Bool driver_func(ScrnInfoPtr pScrn, xorgDriverFuncOp op, pointer ptr) {
  if (op == GET_REQUIRED_HW_INTERFACES) {
    *(CARD32 *)ptr = HW_SKIP_CONSOLE;
    return TRUE;
  }
  return FALSE;
}

static xf86OutputStatus output_detect(xf86OutputPtr output) {
  DriverOutput *driver_output = output->driver_private;

  if (driver_output->connected) {
    return XF86OutputStatusConnected;
  } else {
    return XF86OutputStatusDisconnected;
  }
}

static DisplayModePtr dummy_output_get_modes(xf86OutputPtr output) {
  DisplayModePtr pModes = NULL, pMode, pModeSrc;

  /* copy modes from config */
  for (pModeSrc = output->scrn->modes; pModeSrc; pModeSrc = pModeSrc->next) {
    pMode = xnfcalloc(1, sizeof(DisplayModeRec));
    memcpy(pMode, pModeSrc, sizeof(DisplayModeRec));
    pMode->next = NULL;
    pMode->prev = NULL;
    pMode->name = strdup(pModeSrc->name);
    pModes = xf86ModesAdd(pModes, pMode);
    if (pModeSrc->next == output->scrn->modes) {
      break;
    }
  }
  return pModes;
}

static Bool pre_init(ScrnInfoPtr pScrn, int flags) {
  if (flags & PROBE_DETECT) {
    return TRUE;
  }

  pScrn->monitor = pScrn->confScreen->monitor;
  pScrn->xDpi = 75;
  pScrn->yDpi = 75;

  assert(xf86SetDepthBpp(pScrn, 24, 32, 32, 0));
  assert(pScrn->depth == 24);
  assert(pScrn->bitsPerPixel == 32);

  {
    rgb zeros = {0};
    assert(xf86SetWeight(pScrn, zeros, zeros));
  }
  assert(xf86SetDefaultVisual(pScrn, -1));

  {
    Gamma zeros = {0};
    assert(xf86SetGamma(pScrn, zeros));
  }

  DisplayModePtr mode1 = calloc(1, sizeof(DisplayModeRec));
  mode1->name = strdup("1024x768");
  mode1->Clock = 60;
  mode1->HTotal = 10;
  mode1->VTotal = 100;
  mode1->HDisplay = 1024;
  mode1->VDisplay = 768;

  DisplayModePtr mode2 = calloc(1, sizeof(DisplayModeRec));
  mode2->name = strdup("800x600");
  mode2->Clock = 120;
  mode2->HTotal = 10;
  mode2->VTotal = 100;
  mode2->HDisplay = 800;
  mode2->VDisplay = 600;

  pScrn->modes = mode1;
  xf86ModesAdd(pScrn->modes, mode2);

  xf86SetCrtcForModes(pScrn, 0);

  pScrn->currentMode = pScrn->modes;

  pScrn->displayWidth = WIDTH;

  assert(xf86LoadSubModule(pScrn, "fb"));

  return TRUE;
}

static Bool screen_init(ScreenPtr pScreen, int argc, char **argv) {
  ScrnInfoPtr pScrn = xf86ScreenToScrn(pScreen);

  miClearVisualTypes();
  assert(miSetVisualTypesAndMasks(
      pScrn->depth, miGetDefaultVisualMask(pScrn->depth), pScrn->rgbBits,
      pScrn->defaultVisual, 0xff0000, 0xff00, 0xff));
  assert(miSetPixmapDepths());

  assert(fbScreenInit(pScreen, &driver.pixels, pScrn->virtualX, pScrn->virtualY,
                      pScrn->xDpi, pScrn->yDpi, pScrn->displayWidth,
                      pScrn->bitsPerPixel));
  assert(fbPictureInit(pScreen, 0, 0));

  xf86SetBlackWhitePixels(pScreen);

  static const xf86CrtcConfigFuncsRec crtc_config_funcs = {
      .resize = crtc_config_resize,
  };
  xf86CrtcConfigInit(pScrn, &crtc_config_funcs);

  for (int i = 0; i < NUM_OUTPUTS; i++) {
    static const xf86CrtcFuncsRec crtc_funcs = {
        .dpms = crtc_dpms,
        .lock = crtc_lock,
        .mode_fixup = crtc_mode_fixup,
        .prepare = crtc_stub,
        .mode_set = crtc_mode_set,
        .commit = crtc_stub,
        .gamma_set = crtc_gamma_set,
        .shadow_allocate = crtc_shadow_allocate,
        .destroy = crtc_stub,
    };

    driver.outputs[i].crtc = xf86CrtcCreate(pScrn, &crtc_funcs);
    driver.outputs[i].crtc->driver_private = &driver.outputs[i];
    driver.outputs[i].connected = i == 0;

    char output_name[64];
    sprintf(output_name, "output%u", i);

    static const xf86OutputFuncsRec output_funcs = {
        .dpms = output_dpms,
        .mode_valid = output_mode_valid,
        .mode_fixup = output_mode_fixup,
        .prepare = output_stub,
        .commit = output_stub,
        .mode_set = output_mode_set,
        .detect = output_detect,
        .get_modes = dummy_output_get_modes,
        .destroy = output_stub,
    };

    driver.outputs[i].output =
        xf86OutputCreate(pScrn, &output_funcs, output_name);
    driver.outputs[i].output->possible_crtcs = 1 << i;
    driver.outputs[i].output->possible_clones = 0;
    driver.outputs[i].output->driver_private = &driver.outputs[i];
    driver.outputs[i].output->mm_width = 2000;
    driver.outputs[i].output->mm_height = 1000;

    xf86OutputUseScreenMonitor(driver.outputs[i].output, FALSE);
  }

  xf86CrtcSetSizeRange(pScrn, 1, 1, WIDTH, HEIGHT);

  assert(xf86InitialConfiguration(pScrn, TRUE));

  assert(xf86CrtcScreenInit(pScreen));
  assert(xf86SetDesiredModes(pScrn));

  {
    BoxRec AvailFBArea = {
        .x1 = 0,
        .y1 = 0,
        .x2 = WIDTH,
        .y2 = HEIGHT,
    };
    xf86InitFBManager(pScreen, &AvailFBArea);
  }

  xf86SetBackingStore(pScreen);
  xf86SetSilkenMouse(pScreen);

  assert(miDCInitialize(pScreen, xf86GetPointerScreenFuncs()));
  assert(miCreateDefColormap(pScreen));

  assert(xf86HandleColormaps(pScreen, 1024, pScrn->rgbBits, load_palette, NULL,
                             CMAP_PALETTED_TRUECOLOR |
                                 CMAP_RELOAD_ON_MODE_SWITCH));

  pScreen->SaveScreen = save_screen;

  driver.screen = pScreen;

  return TRUE;
}

static Bool probe(DriverPtr drv, int flags) {
  if (flags & PROBE_DETECT) {
    return FALSE;
  }

  GDevPtr *devSections;
  assert(xf86MatchDevice(DRIVER_NAME, &devSections));

  ScrnInfoPtr pScrn = xf86AllocateScreen(drv, 0);
  assert(pScrn);
  pScrn->driverVersion = DRIVER_VERSION;
  pScrn->driverName = DRIVER_NAME;
  pScrn->name = "Winit Screen";
  pScrn->Probe = probe;
  pScrn->PreInit = pre_init;
  pScrn->ScreenInit = screen_init;
  pScrn->SwitchMode = switch_mode;
  pScrn->EnterVT = enter_vt;
  pScrn->LeaveVT = leave_vt;
  pScrn->ValidMode = valid_mode;
  pScrn->vtSema = TRUE;

  int entityIndex = xf86ClaimNoSlot(drv, 0, devSections[0], TRUE);
  xf86AddEntityToScreen(pScrn, entityIndex);

  free(devSections);

  return TRUE;
}

void video_init(pointer module) {
  static DriverRec driver = {
      .driverVersion = DRIVER_VERSION,
      .driverName = DRIVER_NAME,
      .Probe = probe,
      .driverFunc = driver_func,
  };

  xf86AddDriver(&driver, module, HaveDriverFuncs);
}

void video_connect_second_monitor(uint32_t connected) {
  driver.outputs[1].connected = connected;
  driver.outputs[1].output->mm_width = 20;
  driver.outputs[1].output->mm_height = 20;
  RRSetChanged(driver.screen);
  xf86RandR12TellChanged(driver.screen);
  RRGetInfo(driver.screen, TRUE);
}

void video_get_info(uint32_t *second_crtc, uint32_t *first_output, uint32_t *second_output, uint32_t *small_mode_id, uint32_t *large_mode_id) {
  *second_crtc = driver.outputs[1].crtc->randr_crtc->id;
  *first_output = driver.outputs[0].output->randr_output->id;
  *second_output = driver.outputs[1].output->randr_output->id;
  RROutputPtr output = driver.outputs[0].output->randr_output;
  for (int i = 0; i < output->numModes; i++) {
    RRModePtr mode = output->modes[i];
    ErrorF("width: %d\n", mode->mode.width);
    if (mode->mode.width == 1024) {
      *large_mode_id = mode->mode.id;
    } else {
      *small_mode_id = mode->mode.id;
    }
  }
}
