import { expect, test } from 'bun:test';
import { DATA, DONE, FrameReader, encodeFrame } from '../../public/protocol.js';

test('reads frames across every byte boundary and multiple frames per read', async () => {
	const bytes = new Uint8Array([...encodeFrame(DATA, new Uint8Array([1, 2, 3])), ...encodeFrame(DONE)]);
	for (const size of [1, 2, 4, bytes.length]) {
		const reader = new FrameReader(new ReadableStream({
			start(controller) {
				for (let i = 0; i < bytes.length; i += size) controller.enqueue(bytes.slice(i, i + size));
				controller.close();
			}
		}));
		expect(await reader.read()).toEqual({ type: DATA, data: new Uint8Array([1, 2, 3]) });
		expect(await reader.read()).toEqual({ type: DONE, data: new Uint8Array() });
		expect(await reader.read()).toBeNull();
	}
});

test('rejects truncated headers, payloads, and oversized frames', async () => {
	for (const bytes of [[1, 2], [1, 2, 0, 0, 0, 3], [1, 255, 255, 255, 255]]) {
		const reader = new FrameReader(new ReadableStream({
			start(controller) { controller.enqueue(new Uint8Array(bytes)); controller.close(); }
		}));
		await expect(reader.read()).rejects.toThrow();
	}
});
