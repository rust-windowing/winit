AppKit IME implementation 

Of interest is the [Chromium implementation](https://github.com/chromium/chromium/blob/main/components/remote_cocoa/app_shim/bridged_content_view.mm),
which has enumerated a LOT of tricky edge cases. 
TODO find the Firefox one later and compare them.

First, some context on API surfaces related to IME on AppKit. On the logical server side, the IME application, we have
primarily for server -> client communication
[`IMKTextInput`](https://web.archive.org/web/20081224174350/http://developer.apple.com/documentation/Cocoa/Reference/IMKTextInput_Protocol/Reference/IMKTextInput_Protocol.html)`
(the link points to an archive because the docs have mysteriously disappeared from official apple sources since), along
with the [`IMKServerInput`](https://developer.apple.com/documentation/inputmethodkit/imkserverinput?language=objc)
informal protocol . The client <-> server communications are mediated by internal `TextServicesManager` APIs, which have
an OS service process that likely handles IME server discovery, bringup, IPC routing, etc.

On the client side we have a (subclass of) [`NSView`] that adopts the [`NSTextInputClient`] protocol, and nullable
[`NSTextInputContext`] field on our [`NSView`]

