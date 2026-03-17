## AppKit IME implementation notes

- (for best viewing experience, `cd winit-appkit && cargo doc --document-private-items`, open the `ime` module in the
generated docs)

### Other Implementations
See also these other implementations of IME on AppKit

- [Chromium](<https://github.com/chromium/chromium/blob/c9acbb5436a6676d081f34b372ff1ba28a8f44b7/components/remote_cocoa/app_shim/bridged_content_view.mm>),
- [Firefox](<https://github.com/mozilla-firefox/firefox/blob/4d26a0d0b80b56b95f9f7958a6d16dd4dfb35c1a/widget/cocoa/nsCocoaWindow.mm#L2918>),
- [Java AWT](<https://github.com/openjdk/jdk/blob/ee90f00b3b38b7cf4da340deb48f04bdaee22710/src/java.desktop/macosx/native/libawt_lwawt/awt/AWTView.m>),
- [Qt](<https://github.com/qt/qtbase/blob/e0da96d7f7306592dc05c6fe6a0e81b1c72e7b07/src/plugins/platforms/cocoa/qnsview_complextext.mm>)
    (see also [their input context wrapper](<https://github.com/qt/qtbase/blob/e0da96d7f7306592dc05c6fe6a0e81b1c72e7b07/src/plugins/platforms/cocoa/qcocoainputcontext.mm>))

### Context
First, some context on API surfaces related to IME on AppKit. On the server side, which manages the client's marked text
and interprets user input, we have
[`IMKTextInput`](<https://web.archive.org/web/20081224174350/http://developer.apple.com/documentation/Cocoa/Reference/IMKTextInput_Protocol/Reference/IMKTextInput_Protocol.html>)
(the link points to an archive because the docs have mysteriously disappeared from official apple sources since), which
forwards requests from the server to a client, along with the
[`IMKServerInput`](<https://developer.apple.com/documentation/inputmethodkit/imkserverinput?language=objc>) informal
protocol, which handles events received from clients. The client <-> server communications are mediated by internal
`TextServicesManager` APIs, which include an OS service process that likely handles IME app discovery, bringup, IPC
routing, etc.

On the client side we have a (subclass of) [`NSView`] that adopts the [`NSTextInputClient`] protocol, and a nullable
read-only [`NSTextInputContext`] field on our [`NSView`]. Generally, most methods in [`NSTextInputClient`] are
straightforwards, the difficulty comes from subtle and hard to predict interactions between these methods and the
existing cocoa event handling mechanisms for key presses.

### IME Event Handling
When the application determines a key [`NSEvent`] should be interpreted as a text input and an IME is present (i.e.
[`inputContext`](NSView::inputContext)` != nil`), the event is forwarded to the [`inputContext`](NSView::inputContext)'s
[`handleEvent:`](NSTextInputContext::handleEvent), via [`NSView`]'s override of
[`interpretKeyEvents:`](NSResponder::interpretKeyEvents), which [we are instructed not to override
further](<https://developer.apple.com/documentation/appkit/nsresponder/interpretkeyevents(_:)>).

Calls to functions defined in [`NSTextInputClient`] are dispatched through [`-[NSTextInputClient
doCommandBySelector:]`](NSTextInputClient::doCommandBySelector) which, confusingly, is "method punned" with
[`-[NSStandardKeyBindingResponding doCommandBySelector:]`](NSStandardKeyBindingResponding::doCommandBySelector)
(Objective C nas no namespacing, so the method for selector `doCommandBySelector:` must implement the functionality
required by both protocols).