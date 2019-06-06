export function throwToEscapeEventLoop() {
    throw "Using exceptions for control flow, don't mind me. This isn't actually an error!";
}