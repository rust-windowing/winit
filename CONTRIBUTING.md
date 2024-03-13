# Contribution Guidelines

This document contains guidelines for contributing code to winit. It has to be
followed in order for your patch to be approved and applied.

## Contributing

Anyone can contribute to winit, however given that it's a cross platform
windowing toolkit getting certain changes incorporated upstream could be
challenging.

To save your time it's wise to check already opened [pull requests][prs] and
[issues][issues]. In general, bug fixes and missing implementations are always
accepted, however new API proposals should go into the issue first. When in
doubt contact us on [matrix][matrix] or via opening an issue.

### Submitting your work and handling review

All patches have to be sent on Github as [pull requests][prs]. To simplify your
life during review it's recommended to check the "give contributors write access
to the branch" checkbox.

#### Creating commits

When creating a commit, follow these general rules:
  - Try to limit the first line (title) of the commit message to 50 characters,
    but not more than 60 characters.
  - The commit starts with an uppercase latter and everything else must be
    lowercase, unless you quote a symbol, like `Window`.
  - Use the body of the commit message to actually explain what your patch does
    and why it is useful. Even if your patch is a one line fix, the description
    is not limited in length and may span over multiple paragraphs. Use proper
    English syntax, grammar and punctuation. The body is generally wrapped
    at 72 characters.
  - If you're a downstream user and your fixing your issue, consider to link it
    like `Links: URL` in the end of the commit message.
  - Try to address only one issue/topic per commit.
  - Prefer describing your changes in imperative mood, e.g. *"make xyzzy do
    frotz"* instead of *"[This patch] makes xyzzy do frotz"* or *"[I] changed
    xyzzy to do frotz"*, as if you are giving orders to the codebase to change
    its behaviour.
  - When fixing an issue from https://github.com/rust-windowing/winit/issues,
    ensure to have it include in the end of the commit. See other commits
    on how it's done.
  - When in doubt, follow the format and layout of the recent existing commits.

The maintainers will fixup your commit message when applying the patch, but it's
still recommended to follow the rules above to make the life of maintainers
easier.

If you want to communicate something to reviewers, but it doesn't belong to
the commit message, write that under `--` on PR body. Your PR could look like:

  ```
  TITLE (matches commit title)

  Body (matches commit body)

  --

  Your arbitrary message goes here.

  Various checkboxes PR template required you to fill.
  ```

#### Handling review

During the review process certain events could require an action from your side,
to communication more efficient the common patterns and reactions are described
below.

_Event:_ The CI fails to build, but it looks like not your fault. Not
communicating so, could result into maintainers not looking into your patch,
unless they CI that CI pass.\
_Desired behavior:_ Write a message saying roughly the following "The CI failure
is unrelated", so maintainers will fix it for you.

_Event:_ Collaborator requested review on your PR.\
_Desired behavior:_ Once you address the request, you _should_ re-request review
with the github's UI. If you don't agree with what maintainer suggested, you
should object that and re-request the review. That will indicate that the
_ball_ is on maintainer's side.

_Event:_ You've opened a PR, but maintainer shortly after commented that they
want to work on that _themselves_.\
_Desired behavior:_ Discusses with maintainer their plans if they were not
outlined in the initial response, because such response means that they
are not interested in reviewing your code. Such thing could happen when
underestimating complexity of the task you're solving or when your patch
mandate certain downstream designs.

[prs]: https://github.com/rust-windowing/winit/pulls
[issues]: https://github.com/rust-windowing/winit/issues
[matrix]: https://matrix.to/#/#rust-windowing:matrix.org

## Maintainers

Winit has plenty of maintainers with different backgrounds, experience level,
and reasons to be winit maintainer in the first place. To ensure that winit's
code quality is not decreasing over time and to make it easier to teach new
maintainers the _winit way of doing things_ the common policies and routines
are defined in this section.

The current maintainers for each platform are listed in [this file][CODEOWNERS].

### Contributions handling

The maintainer must ensure that the external contributions meet the winit's
quality standards. If it's not, it **is maintainer's responsibility** to bring
it on par, which includes:

  - Decline the patch if it tries to achieve short term goals instead of solving
    the problem in general. In such case maintainer must communicate that **as
    early as possible**, so contributors don't spend their time on something
    that won't be accepted in the end.
  - Ensure that formatting is consistent and `CHANGELOG` messages are clear
    for the end users.
  - Improve the commit message, so it'll be easier for other maintainers to
    understand the motivation without going through all the discussions on the
    particular patch/PR.
  - Ensure that the proposed patch doesn't break platform parity. If the
    breakage is desired by contributor, an issue should be opened to discuss
    with other maintainers before merging.
  - Always fix CI issues before merging if they don't originate from the
    submitted work.

However, maintainer must always give a leeway for external contributors, so they
don't feel discouraged contributing, for example:

  - Suggest a patch to resolve style issues, if it's the only issue with the
    submitted work. Keep in mind that pushing the resolution yourself is not
    desired, because contributor might not agree with what you did.
  - Be more explicit on how things should be done if you doesn't like the
    approach.
  - Suggest to finish PR for them if they're absent for a while and you need the
    proposed changes to move forward with something. In such case maintainer
    must preserve attribution with `Co-authored-by`, `Suggested-by`, or keep
    the original commiter.
  - Rebase their work for them when massive changes to winit codebase were
    introduced.

When reviewing code of other maintainers all of the above is on the maintainer
who submitted the patch. Interested maintainers could help push the work over
the finish line, but teaching other maintainers should be preferred.

When contributor is _regular_ in winit, the maintainer should slowly start
requiring contributor to match *maintainer* quality standards when writing
patches and commit messages.

### Contributing

When submitting a patch maintainer should follow the general contributing
guidelines, however all soft rules (e.g `Try to`), become strict.

To make life simpler for other maintainers it's suggested to create your branch
under the project repository instead of your own fork. The naming scheme is
`github_user_name/branch_name`. Doing so will make your work easier to rebase
for other maintainers when you're absent.

### Release process

Given that winit is a widely used library, we should be able to make a patch
releases at any time we want without blocking the development of new features.

To achieve these goals, a new branch is created for every new release. Releases
and later patch releases are committed and tagged in this branch.

The exact steps for an exemplary `0.2.0` release might look like this:
  1. Initially, the version on the latest master is `0.1.0`
  2. A new `v0.2.x` branch is created for the release
  3. In the branch, the version is bumped to `v0.2.0`
  4. The new commit in the branch is tagged `v0.2.0`
  5. The version is pushed to crates.io
  6. A GitHub release is created for the `v0.2.0` tag
  7. On master, the version is bumped to `0.2.0`, and the CHANGELOG is updated

When doing a patch release, the process is similar:
  1. Initially, the version of the latest release is `0.2.0`
  2. Checkout the `v0.2.x` branch
  3. Cherry-pick the required non-breaking changes into the `v0.2.x`
  4. Follow steps 3-7 of the regular release example

[CODEOWNERS]: .github/CODEOWNERS
