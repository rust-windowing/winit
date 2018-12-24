# Winit Contributing Guidelines

## Scope

Winit aims to provide a generic platform abstracting the main graphic platforms (Windows, macOS, X11,
Wayland, Android, iOS and the web platform via Emscripten).

Most platforms expose capabilities that cannot be meaningfully transposed to the others. Winit does not
aim to support every single functionality of every platform, but rather to abstract the set of
capabilities that is common to all platforms. In this context, APIs exposed in winit can be split into
different "support levels":

- Tier 1: features which are in the main scope of winit. They are part of the common API of winit, and
  are taken care of by the maintainers. Any part of these features that is not working correctly is
  considered a bug in winit.
- Tier 2: some platform-specific features can be sufficiently fundamental to the platform that winit can
  integrate support for them in the platform-specific part of the API. These features are not considered
  directly handled by the maintainers of winit. If you have a strong incentive to have such a feature
  integrated in winit, consider implementing it and proposing yourself to maintain it in the future.
- Tier 3: these features are not directly exposed by winit, but rather can be implemented using the
  raw handles to the underlying platform that winit exposes. If your feature of interest is rather
  niche, this is probably where it belongs.

The exact list of supported Tier 1 features is tracked in this issue:
[#252](https://github.com/tomaka/winit/issues/252).

## Reporting an issue

When reporting an issue, in order to help the maintainers understand what the problem is, please make
your description of the issue as detailed as possible:

- if it is a bug, please provide clear explanation of what happens, what should happen, and how to
  reproduce the issue, ideally by providing a minimal program exhibiting the problem
- if it is a feature request, please provide a clear argumentation about why you believe this feature
  should be supported by winit

## Making a pull request

When making a code contribution to winit, before opening your pull request, please make sure that:

- you tested your modifications on all the platforms impacted, or if not possible detail which platforms
  were not tested, and what should be tested, so that a maintainer or another contributor can test them
- you updated any relevant documentation in winit
- you left comments in your code explaining any part that is not straightforward, so that the
  maintainers and future contributors don't have to try to guess what your code is supposed to do
- your PR adds an entry to the changelog file if the introduced change is relevant to winit users

Once your PR is open, you can ask for review by a maintainer of your platform. Winit's merging policy
is that a PR must be approved by at least two maintainers of winit before being merged, including
at least a maintainer of the platform (a maintainer making a PR themselves counts as approving it).

## Maintainers & Testers

Winit is managed by several people, each with their specialities, and each maintaining a subset of the
backends of winit. As such, depending on your platform of interest, your contacts will be different.

This table summarizes who can be contacted in which case, with the following legend:

- `M`: is a main maintainer for this platform
- `R`: can review code for this platform
- `T`: has the ability of testing the platform
- ` `: knows nothing of this platform

| Platform | Windows | macOS | X11 | Wayland | Android | iOS | Emscripten |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| @francesca64 | R | M | M |  | M | R | |
| @mitchmindtree | T |  | T | T |  |  |  |
| @Osspial | M |  | T | T | T |  | T |
| @vberger |  |  | T | M |  |  |  |
| @mtak- |  | T |  |  | T | M |  |
