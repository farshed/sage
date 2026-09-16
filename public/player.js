export class SpeechPlayer {
	constructor(context, sampleRate, signal, onStarted) {
		this.signal = signal;
		this.sampleRate = sampleRate;
		this.sent = 0;
		this.consumed = 0;
		this.waiters = [];
		this.stopped = false;
		this.node = new AudioWorkletNode(context, 'speech-playback', {
			numberOfInputs: 0,
			numberOfOutputs: 1,
			outputChannelCount: [1],
			processorOptions: { sampleRate }
		});
		this.finished = new Promise((resolve, reject) => {
			this.resolve = resolve;
			this.reject = reject;
		});
		void this.finished.catch(() => {});
		this.node.port.onmessage = ({ data }) => {
			if (data.type === 'started') onStarted();
			if (data.type === 'progress') {
				this.consumed = data.consumed;
				this.wake();
			}
			if (data.type === 'ended') {
				this.resolve();
				this.stop();
			}
		};
		this.node.onprocessorerror = () => this.stop(new Error('Audio playback failed.'));
		this.cancel = () => this.stop(signal.reason);
		signal.addEventListener('abort', this.cancel, { once: true });
		this.node.connect(context.destination);
		if (signal.aborted) this.cancel();
	}

	wake() {
		for (const resolve of this.waiters.splice(0)) resolve();
	}

	async push(bytes) {
		while (this.sent - this.consumed > this.sampleRate * 8 && !this.stopped) {
			await new Promise((resolve) => this.waiters.push(resolve));
		}
		this.signal.throwIfAborted();
		if (this.stopped) throw new Error('Audio playback stopped.');
		if (bytes.byteLength % 4 !== 0) throw new Error('Invalid audio packet.');
		const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
		const samples = new Float32Array(bytes.byteLength / 4);
		for (let i = 0; i < samples.length; i++) samples[i] = view.getFloat32(i * 4, true);
		this.sent += samples.length;
		this.node.port.postMessage({ type: 'audio', samples }, [samples.buffer]);
	}

	end() {
		this.node.port.postMessage({ type: 'end' });
		return this.finished;
	}

	stop(error) {
		if (this.stopped) return;
		this.stopped = true;
		this.signal.removeEventListener('abort', this.cancel);
		this.node.port.postMessage({ type: 'cancel' });
		this.node.disconnect();
		this.node.port.close();
		this.wake();
		if (error) this.reject(error);
		else this.resolve();
	}
}
