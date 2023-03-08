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

- your patch builds with Winit's minimal supported rust version - Rust 1.64.
- you tested your modifications on all the platforms impacted, or if not possible detail which platforms
  were not tested, and what should be tested, so that a maintainer or another contributor can test them
- you updated any relevant documentation in winit
- you left comments in your code explaining any part that is not straightforward, so that the
  maintainers and future contributors don't have to try to guess what your code is supposed to do
- your PR adds an entry to the changelog file if the introduced change is relevant to winit users.

  You needn't worry about the added entry causing conflicts, the maintainer that merges the PR will
  handle those for you when merging (see below).
- if your PR affects the platform compatibility of one or more features or adds another feature, the
  relevant sections in [`FEATURES.md`](https://github.com/rust-windowing/winit/blob/master/FEATURES.md#features)
  should be updated.

Once your PR is open, you can ask for review by a maintainer of your platform. Winit's merging policy
is that a PR must be approved by at least two maintainers of winit before being merged, including
at least a maintainer of the platform (a maintainer making a PR themselves counts as approving it).

Once your PR is deemed ready, the merging maintainer will take care of resolving conflicts in
`CHANGELOG.md` (but you must resolve other conflicts yourself). Doing this requires that you check the
"give contributors write access to the branch" checkbox when creating the PR.

## Maintainers & Testers

The current maintainers are listed in the [CODEOWNERS](.github/CODEOWNERS) file.

If you are interested in being pinged when testing is needed for a certain platform, please add yourself to the [Testers and Contributors](https://github.com/rust-windowing/winit/wiki/Testers-and-Contributors) table!

## Release process

Given that winit is a widely used library we should be able to make a patch
releases at any time we want without blocking the development of new features.

To achieve these goals, a new branch is created for every new release. Releases
and later patch releases are committed and tagged in this branch.

The exact steps for an exemplary `0.2.0` release might look like this:
  1. Initially the version on the latest master is `0.1.0`
  2. A new `v0.2.x` branch is created for the release
  3. In the branch, the version is bumped to `v0.2.0`
  4. The new commit in the branch is tagged `v0.2.0`
  5. The version is pushed to crates.io
  6. A GitHub release is created for the `v0.2.0` tag
  7. On master, the version is bumped to `0.2.0` and the CHANGELOG is updated
  
When doing a patch release the process is similar:
  1. Initially the version of the latest release is `0.2.0`
  2. Checkout the `v0.2.x` branch
  3. Cherry-pick the required non-breaking changes into the `v0.2.x`
  4. Follow steps 3-7 of the regular release example
