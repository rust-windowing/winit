onmessage = (event) => {
	const [port, timeout] = event.data as [MessagePort, number]
	const f = () => port.postMessage(undefined)

	if ('scheduler' in globalThis) {
		void globalThis.scheduler.postTask(f, { delay: timeout })
	} else {
		setTimeout(f, timeout)
	}
}
