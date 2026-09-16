import { PCMBuffer } from './audio-buffer.js';

class SpeechPlayback extends AudioWorkletProcessor {
	constructor(options) {
		super();
		this.audio = new PCMBuffer(options.processorOptions.sampleRate, sampleRate);
		this.reportedStarted = false;
		this.lastReported = 0;
		this.cancelled = false;
		this.port.onmessage = ({ data }) => {
			if (data.type === 'audio') this.audio.push(data.samples);
			if (data.type === 'end') this.audio.ended = true;
			if (data.type === 'cancel') this.cancelled = true;
		};
	}

	process(_inputs, outputs) {
		if (this.cancelled) return false;
		const written = this.audio.render(outputs[0][0]);
		if (written && !this.reportedStarted) {
			this.reportedStarted = true;
			this.port.postMessage({ type: 'started' });
		}
		const finished = this.audio.ended && this.audio.available === 0;
		if (this.audio.consumed - this.lastReported >= 1024 || finished) {
			this.lastReported = this.audio.consumed;
			this.port.postMessage({ type: 'progress', consumed: this.audio.consumed });
		}
		if (finished) this.port.postMessage({ type: 'ended' });
		return !finished;
	}
}

registerProcessor('speech-playback', SpeechPlayback);
