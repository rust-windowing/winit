onmessage = event => {
    const [port, timeout] = event.data
    const f = () => port.postMessage(undefined)

    if ('scheduler' in this) {
        scheduler.postTask(f, { delay: timeout })
    } else {
        setTimeout(f, timeout)
    }
}
