export const READY = 0;
export const DATA = 1;
export const DONE = 2;
export const ERROR = 3;
export const METADATA = 4;
export const MAX_FRAME_BYTES = 32 * 1024 * 1024;

export function encodeFrame(type, data) {
	data ??= new Uint8Array();
	if (data.byteLength > MAX_FRAME_BYTES) throw new Error('Frame is too large');
	const frame = new Uint8Array(5 + data.byteLength);
	frame[0] = type;
	new DataView(frame.buffer).setUint32(1, data.byteLength, true);
	frame.set(data, 5);
	return frame;
}

export class FrameReader {
	constructor(stream) {
		this.reader = stream.getReader();
		this.buffer = new Uint8Array();
		this.offset = 0;
	}

	async readBytes(length, allowEOF = false) {
		const output = new Uint8Array(length);
		let written = 0;
		while (written < length) {
			if (this.offset === this.buffer.length) {
				const { value, done } = await this.reader.read();
				if (done) {
					if (allowEOF && written === 0) return null;
					throw new Error('Incomplete stream frame');
				}
				this.buffer = value;
				this.offset = 0;
			}
			const count = Math.min(length - written, this.buffer.length - this.offset);
			output.set(this.buffer.subarray(this.offset, this.offset + count), written);
			this.offset += count;
			written += count;
		}
		return output;
	}

	async read() {
		const header = await this.readBytes(5, true);
		if (!header) return null;
		const length = new DataView(header.buffer).getUint32(1, true);
		if (length > MAX_FRAME_BYTES) throw new Error('Frame is too large');
		const data = await this.readBytes(length);
		if (!data) throw new Error('Missing frame payload');
		return { type: header[0], data };
	}

	cancel() {
		return this.reader.cancel();
	}
}
