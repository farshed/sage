import { afterEach, expect, test } from 'bun:test';
import { SpeechPlayer } from '../../public/player.js';

const original = globalThis.AudioWorkletNode;
let node: FakeNode;

class FakeNode {
	connected = false;
	messages: any[] = [];
	port = {
		onmessage: null as null | ((event: { data: unknown }) => void),
		postMessage: (data: unknown) => { this.messages.push(data); },
		close() {}
	};
	onprocessorerror?: () => void;
	constructor() { node = this; }
	connect() { this.connected = true; }
	disconnect() { this.connected = false; }
	receive(data: unknown) { this.port.onmessage?.({ data }); }
}

function player() {
	globalThis.AudioWorkletNode = FakeNode as unknown as typeof AudioWorkletNode;
	const abort = new AbortController();
	let started = false;
	const playback = new SpeechPlayer({ destination: {} }, 24000, abort.signal, () => { started = true; });
	return { playback, abort, started: () => started };
}

afterEach(() => { globalThis.AudioWorkletNode = original; });

test('feeds audio before completion and waits for the worklet to finish playback', async () => {
	const { playback, started } = player();
	await playback.push(new Uint8Array(new Float32Array([0.5, -0.5]).buffer));
	expect(node.messages[0].type).toBe('audio');
	expect([...node.messages[0].samples]).toEqual([0.5, -0.5]);
	node.receive({ type: 'started' });
	expect(started()).toBe(true);
	let finished = false;
	const completion = playback.end().then(() => { finished = true; });
	await Promise.resolve();
	expect(finished).toBe(false);
	node.receive({ type: 'ended' });
	await completion;
	expect(node.connected).toBe(false);
});

test('bounds queued audio and resumes when playback makes room', async () => {
	const { playback } = player();
	try {
		await playback.push(new Uint8Array((24000 * 8 + 1) * 4));
		let accepted = false;
		const next = playback.push(new Uint8Array(4)).then(() => { accepted = true; });
		await Promise.resolve();
		expect(accepted).toBe(false);
		node.receive({ type: 'progress', consumed: 24000 });
		await next;
		expect(accepted).toBe(true);
	} finally { playback.stop(); }
});

test('cancellation stops playback and releases a blocked packet', async () => {
	const { playback, abort } = player();
	await playback.push(new Uint8Array((24000 * 8 + 1) * 4));
	const blocked = playback.push(new Uint8Array(4));
	abort.abort();
	await expect(blocked).rejects.toThrow();
	await expect(playback.finished).rejects.toThrow();
	expect(node.connected).toBe(false);
});

test('worklet failures reject playback and release audio resources', async () => {
	const { playback } = player();
	node.onprocessorerror?.();
	await expect(playback.finished).rejects.toThrow('Audio playback failed');
	expect(node.connected).toBe(false);
});
