export class PCMBuffer {
	constructor(inputRate, outputRate) {
		this.ratio = inputRate / outputRate;
		this.chunks = [];
		this.offset = 0;
		this.position = 0;
		this.available = 0;
		this.consumed = 0;
		this.ended = false;
		this.started = false;
		this.startThreshold = Math.ceil(inputRate * 0.08);
	}

	push(samples) {
		if (samples.length) {
			this.chunks.push(samples);
			this.available += samples.length;
		}
	}

	peek(next = false) {
		const chunk = this.chunks[0];
		if (!next) return chunk[this.offset];
		return chunk[this.offset + 1] ?? this.chunks[1]?.[0] ?? chunk[this.offset];
	}

	discard(count) {
		count = Math.min(count, this.available);
		this.available -= count;
		this.consumed += count;
		while (count > 0) {
			const remaining = this.chunks[0].length - this.offset;
			if (count < remaining) {
				this.offset += count;
				return;
			}
			count -= remaining;
			this.chunks.shift();
			this.offset = 0;
		}
	}

	render(output) {
		output.fill(0);
		if (!this.started && (this.available >= this.startThreshold || this.ended)) {
			this.started = this.available > 0;
		}
		if (!this.started) return 0;
		let written = 0;
		for (; written < output.length && this.available > 0; written++) {
			if (this.available < 2 && !this.ended) break;
			const current = this.peek();
			output[written] = current + (this.peek(true) - current) * this.position;
			this.position += this.ratio;
			const count = Math.floor(this.position);
			this.position -= count;
			this.discard(count);
		}
		return written;
	}
}
