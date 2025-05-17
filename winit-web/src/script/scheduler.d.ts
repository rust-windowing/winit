declare global {
	// eslint-disable-next-line no-var
	var scheduler: Scheduler
}

export interface Scheduler {
	postTask<T>(callback: () => T | PromiseLike<T>, options?: SchedulerPostTaskOptions): Promise<T>
}

export interface SchedulerPostTaskOptions {
	delay?: number
}
