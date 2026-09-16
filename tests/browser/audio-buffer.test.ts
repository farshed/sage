import { expect, test } from 'bun:test';
import { PCMBuffer } from '../../public/audio-buffer.js';

test('plays before the stream ends and waits for a small initial buffer', () => {
	const audio = new PCMBuffer(24000, 48000);
	const output = new Float32Array(128);
	audio.push(new Float32Array(1000).fill(0.25));
	expect(audio.render(output)).toBe(0);
	audio.push(new Float32Array(1000).fill(0.25));
	expect(audio.render(output)).toBe(128);
	expect(output.every((sample) => sample === 0.25)).toBe(true);
	expect(audio.ended).toBe(false);
});

test('keeps audio duration across supported output rates', () => {
	for (const rate of [16000, 24000, 44100, 48000]) {
		const audio = new PCMBuffer(24000, rate);
		audio.push(new Float32Array(24000).fill(0.5));
		audio.ended = true;
		let count = 0;
		while (audio.available) count += audio.render(new Float32Array(128));
		expect(Math.abs(count - rate)).toBeLessThanOrEqual(1);
		expect(audio.consumed).toBe(24000);
	}
});

test('interpolates across packet boundaries without dropping samples', () => {
	const audio = new PCMBuffer(24000, 48000);
	audio.push(new Float32Array([0, 1]));
	audio.push(new Float32Array([0, -1]));
	audio.ended = true;
	const output = new Float32Array(8);
	expect(audio.render(output)).toBe(8);
	expect([...output]).toEqual([0, 0.5, 1, 0.5, 0, -0.5, -1, -1]);
	expect(audio.available).toBe(0);
});

test('survives underflow and drains short final chunks', () => {
	const audio = new PCMBuffer(24000, 24000);
	audio.push(new Float32Array(2000).fill(0.5));
	audio.render(new Float32Array(4096));
	expect(audio.available).toBe(1);
	const gap = new Float32Array(128).fill(1);
	expect(audio.render(gap)).toBe(0);
	expect(gap.every((sample) => sample === 0)).toBe(true);
	audio.push(new Float32Array([0.25, 0.125]));
	audio.ended = true;
	const tail = new Float32Array(128);
	expect(audio.render(tail)).toBe(3);
	expect([...tail.slice(0, 3)]).toEqual([0.5, 0.25, 0.125]);
});
