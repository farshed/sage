import { DATA, DONE, ERROR, METADATA, FrameReader } from './protocol.js';
import { SpeechPlayer } from './player.js';

let messages = [];
let context;
let workletReady;
let activeTurn;
let state = 'idle';

const chatBtn = document.getElementById('chatBtn');
const helperText = document.getElementById('helperText');
const indicators = { idle: '🎙️', listening: '👂', loading: '⏳', speaking: '🗣️' };
const labels = {
	idle: 'Click to speak',
	listening: 'Listening... Click to finish',
	loading: 'Thinking... Click to cancel',
	speaking: 'Speaking... Click to cancel'
};

function setState(next) {
	state = next;
	chatBtn.innerText = indicators[next];
	helperText.textContent = labels[next];
	chatBtn.setAttribute('aria-label', labels[next]);
	chatBtn.classList.toggle('hover:bg-red-500', next === 'speaking' || next === 'loading');
}

chatBtn.addEventListener('click', () => {
	if (state === 'idle') void record();
	else if (state === 'listening' && activeTurn?.recorder?.state === 'recording') {
		activeTurn.stoppedAt = performance.now();
		setState('loading');
		activeTurn.recorder.stop();
	} else if (state === 'loading' || state === 'speaking') {
		activeTurn?.abort.abort();
		activeTurn?.player?.stop();
		activeTurn = undefined;
		setState('idle');
	}
});

async function record() {
	const turn = { abort: new AbortController() };
	activeTurn = turn;
	setState('listening');
	chatBtn.disabled = true;
	try {
		if (!context) context = new AudioContext();
		await context.resume();
		if (!workletReady) {
			workletReady = context.audioWorklet.addModule('/public/audio-worklet.js').catch((error) => {
				workletReady = undefined;
				throw error;
			});
		}
		await workletReady;
		turn.stream = await navigator.mediaDevices.getUserMedia({ audio: true });
		turn.recorder = new MediaRecorder(turn.stream);
		const chunks = [];
		turn.recorder.ondataavailable = ({ data }) => { if (data.size) chunks.push(data); };
		turn.recorder.onerror = () => fail(turn, new Error('Microphone recording failed.'));
		turn.recorder.onstop = async () => {
			turn.stream.getTracks().forEach((track) => track.stop());
			try {
				turn.abort.signal.throwIfAborted();
				const recording = new Blob(chunks, { type: turn.recorder.mimeType });
				const wav = await encode(recording);
				turn.abort.signal.throwIfAborted();
				await sendAudio(wav, turn);
			} catch (error) {
				fail(turn, error);
			} finally {
				turn.player?.stop();
				if (activeTurn === turn) {
					activeTurn = undefined;
					setState('idle');
				}
			}
		};
		turn.recorder.start();
	} catch (error) {
		fail(turn, error);
	} finally {
		chatBtn.disabled = false;
	}
}

function fail(turn, error) {
	const cancelled = turn.abort.signal.aborted;
	turn.abort.abort(error);
	turn.stream?.getTracks().forEach((track) => track.stop());
	turn.player?.stop(error);
	if (activeTurn !== turn) return;
	activeTurn = undefined;
	setState('idle');
	if (!cancelled) {
		console.error(error);
		helperText.textContent = error instanceof Error ? error.message : 'The voice response failed. Try again.';
	}
}

async function encode(blob) {
	const buffer = await context.decodeAudioData(await blob.arrayBuffer());
	return new Promise((resolve, reject) => {
		const worker = new Worker('/public/wav-worker.js');
		worker.onmessage = ({ data }) => {
			worker.terminate();
			resolve(new Blob([data.buffer], { type: 'audio/wav' }));
		};
		worker.onerror = () => {
			worker.terminate();
			reject(new Error('Could not encode the recording.'));
		};
		const pcmArrays = Array.from({ length: buffer.numberOfChannels }, (_, i) => buffer.getChannelData(i).slice());
		worker.postMessage({ pcmArrays, config: { sampleRate: buffer.sampleRate } }, pcmArrays.map((pcm) => pcm.buffer));
	});
}

async function sendAudio(wav, turn) {
	const form = new FormData();
	form.append('file', wav);
	form.append('messages', JSON.stringify(messages));
	const response = await fetch('/chat', { method: 'POST', body: form, signal: turn.abort.signal });
	if (!response.ok) {
		const data = await response.json().catch(() => ({}));
		throw new Error(data.error || `Request failed (${response.status}).`);
	}
	if (!response.body) throw new Error('Audio streaming is unavailable.');
	const reader = new FrameReader(response.body);
	const decoder = new TextDecoder();
	let completed = false;
	try {
		while (true) {
			const frame = await reader.read();
			if (!frame) break;
			turn.abort.signal.throwIfAborted();
			if (frame.type === METADATA) {
				const format = JSON.parse(decoder.decode(frame.data));
				if (turn.player || format.sampleRate !== 24000 || format.channels !== 1 || format.encoding !== 'f32le') {
					throw new Error('Unsupported speech audio format.');
				}
				turn.player = new SpeechPlayer(context, format.sampleRate, turn.abort.signal, () => {
					if (activeTurn !== turn) return;
					setState('speaking');
					console.log(`First audio played after ${Math.round(performance.now() - turn.stoppedAt)} ms`);
				});
				void turn.player.finished.catch((error) => fail(turn, error));
			} else if (frame.type === DATA) {
				if (!turn.player) throw new Error('Missing speech audio format.');
				await turn.player.push(frame.data);
			} else if (frame.type === ERROR) {
				throw new Error(JSON.parse(decoder.decode(frame.data)).error);
			} else if (frame.type === DONE) {
				if (!turn.player) throw new Error('No speech was generated.');
				const result = JSON.parse(decoder.decode(frame.data));
				messages = result.messages;
				console.log('Server timings:', result.timings);
				completed = true;
				await turn.player.end();
				break;
			} else throw new Error('Unexpected speech packet.');
		}
		if (!completed) throw new Error('The voice response was interrupted. Try again.');
	} finally {
		await reader.cancel().catch(() => {});
	}
}

setState('idle');
