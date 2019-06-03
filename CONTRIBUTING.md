# PLEASE MAKE PRs AGAINST THE `eventloop-2.0` BRANCH.

All development work for our next version is being done against that branch. Refer to [#459](https://github.com/rust-windowing/winit/issues/459) and [the associated milestone](https://github.com/rust-windowing/winit/milestone/2) for details on what that branch changes.

# Winit Contributing Guidelines

## Scope
[See `FEATURES.md`](./FEATURES.md). When requesting or implementing a new Winit feature, you should
consider whether or not it's directly related to window creation or input handling. If it isn't, it
may be worth creating a separate crate that extends Winit's API to add that functionality.


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
- if your PR affects the platform compatibility of one or more features or adds another feature, the
  relevant sections in [`FEATURES.md`](https://github.com/rust-windowing/winit/blob/master/FEATURES.md#features)
  should be updated.

Once your PR is open, you can ask for review by a maintainer of your platform. Winit's merging policy
is that a PR must be approved by at least two maintainers of winit before being merged, including
at least a maintainer of the platform (a maintainer making a PR themselves counts as approving it).

## Maintainers & Testers

Winit is managed by several people, each with their specialities, and each maintaining a subset of the
backends of winit. As such, depending on your platform of interest, your contacts will be different.

This table summarizes who can be contacted in which case, with the following legend:

- `M` - Maintainer: is a main maintainer for this platform
- `C` - Collaborator: can review code and address issues on this platform
- `T` - Tester: has the ability of testing the platform
- ` `: knows nothing of this platform

| Platform            | Windows | macOS | X11   | Wayland | Android | iOS   | Emscripten |
| :---                | :---:   | :---: | :---: | :---:   | :---:   | :---: | :---:      |
| @mitchmindtree      | T       |       | T     | T       |         |       |            |
| @Osspial            | M       |       | T     | T       | T       |       | T          |
| @vberger            |         |       | T     | M       |         |       |            |
| @mtak-              |         | T     |       |         | T       | M     |            |
