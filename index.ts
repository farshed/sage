import { Elysia } from 'elysia';
import { streamText } from 'ai';
import { ollama } from 'ollama-ai-provider';
import { openai } from '@ai-sdk/openai';
import { deepseek } from '@ai-sdk/deepseek';
import { anthropic } from '@ai-sdk/anthropic';
import { createTogetherAI } from '@ai-sdk/togetherai';
import fs from 'fs/promises';
import { spawn } from 'node:child_process';
import { createWriteStream, type WriteStream } from 'fs';
import open from 'open';

const KOKORO = './kokoro/target/release/koko';
const WHISPER = './whisper/target/release/whisper';
const TMP = './tmp';

const WHISPER_MODEL = './models/ggml-medium.en.bin';

const provider = getLLMProvider();

const app = new Elysia();

app.get('/', Bun.file('./public/ui.html'));
app.get('/public/:filename', ({ params }) => Bun.file(`./public/${params.filename}`));
app.get('/play/:filename', ({ params }) => Bun.file(`./tmp/output/${params.filename}`));

app.post('/chat', async ({ body }) => {
	const { id, file, messages: history } = body as any;
	const prevMessages = JSON.parse(history);

	await Bun.write(`${TMP}/input/${id}.wav`, file);

	const tscInit = performance.now();
	const transcript = await transcribe(id);
	const tscTook = Math.round(performance.now() - tscInit);
	console.log(`[took ${tscTook}ms]`, 'Transcript: ', transcript);

	if (!transcript) {
		return { messages: prevMessages, error: 'No speech was detected!' };
	}

	const prompt = { role: 'user', content: transcript };
	const messages = [...prevMessages, prompt];

	const { textStream } = await streamText({
		model: provider,
		system: 'Respond in plain-text only. Keep your responses short and succint.',
		messages
	});

	const ttsStream = new TTSStream(id);
	const message = await processTextStream(textStream, ttsStream);
	console.log('Response: ', message);

	return {
		audio: `/play/${id}.wav`,
		messages: [...messages, { role: 'assistant', content: message }]
	};
});

app.listen(3000, () => {
	open('http://localhost:3000').catch(() =>
		console.log('Navigate to http://localhost:3000 in your browser')
	);
});

function getLLMProvider() {
	const {
		OLLAMA_MODEL,
		OPENAI_API_KEY,
		OPENAI_MODEL,
		DEEPSEEK_API_KEY,
		DEEPSEEK_MODEL,
		ANTHROPIC_API_KEY,
		ANTHROPIC_MODEL,
		TOGETHER_API_KEY,
		TOGETHER_MODEL
	} = process.env;

	if (OLLAMA_MODEL) return ollama(OLLAMA_MODEL);
	if (DEEPSEEK_API_KEY) return deepseek(DEEPSEEK_MODEL || 'deepseek-chat');
	if (ANTHROPIC_API_KEY) return anthropic(ANTHROPIC_MODEL || 'claude-3-5-haiku-20241022');
	if (OPENAI_API_KEY) return openai(OPENAI_MODEL || 'gpt-4-turbo');

	if (TOGETHER_API_KEY) {
		const togetherai = createTogetherAI({
			apiKey: TOGETHER_API_KEY
		});
		return togetherai(TOGETHER_MODEL || 'deepseek-ai/DeepSeek-V3');
	}

	throw new Error(
		'Environment variable OLLAMA_MODEL and TOGETHER_API_KEY not found. You need to specify one.'
	);
}

function transcribe(id: string): Promise<string> {
	const whisperProcess = spawn(WHISPER, [WHISPER_MODEL, id]);

	return new Promise((resolve, reject) => {
		whisperProcess.stdout.on('end', () => {
			fs.readFile(`${TMP}/transcripts/${id}.txt`)
				.then((buffer) => resolve(cleanTranscript(buffer.toString())))
				.catch(reject);
		});
	});
}

class TTSStream {
	path: string;
	output: WriteStream;
	kokoroProcess = spawn(KOKORO, ['--stream'], { stdio: 'pipe' });

	constructor(id: string) {
		this.path = `${TMP}/output/${id}.wav`;
		this.output = createWriteStream(this.path);

		this.kokoroProcess.stdout.on('data', (chunk) => this.output.write(chunk));
	}

	write(input: string) {
		this.kokoroProcess.stdin.write(input + '\n');
	}

	close() {
		return new Promise((resolve) => {
			this.kokoroProcess.stdout.on('end', () => {
				this.output.end();
				resolve(undefined);
			});
			this.kokoroProcess.stdin.end();
		});
	}
}

async function processTextStream(textStream: AsyncIterable<string>, ttsStream: TTSStream) {
	let buffer = '';
	let message = '';

	for await (const textChunk of textStream) {
		buffer += textChunk;
		message += textChunk;

		const sentenceEndIdx = buffer.search(/[.!?]\s/);
		if (sentenceEndIdx !== -1) {
			const sentence = buffer
				.slice(0, sentenceEndIdx + 1)
				.trim()
				.replace(/\*{1,2}([^*]+)\*{1,2}/g, '$1');
			buffer = buffer.slice(sentenceEndIdx + 1).trim();
			ttsStream.write(sentence + '\n');
		}
	}

	if (buffer.trim()) {
		ttsStream.write(buffer.trim() + '\n');
	}
	await ttsStream.close();

	return message;
}

function cleanTranscript(input: string) {
	return input
		.replace(/\[.*?\]/g, '')
		.trim()
		.replace(/\s+/g, ' ');
}
