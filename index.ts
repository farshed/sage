import { Elysia } from 'elysia';
import { streamText } from 'ai';
import { ollama } from 'ollama-ai-provider';
import { openai } from '@ai-sdk/openai';
import { deepseek } from '@ai-sdk/deepseek';
import { anthropic } from '@ai-sdk/anthropic';
import { togetherai } from '@ai-sdk/togetherai';
import fs from 'fs/promises';
import { createWriteStream, type WriteStream } from 'fs';
import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process';
import open from 'open';

const CONFIG = {
	TMP: './tmp',
	KOKORO: './kokoro/target/release/koko',
	WHISPER: './whisper/target/release/whisper',
	WHISPER_MODEL: './models/ggml-large-v3-turbo.bin',
	SYSTEM_PROMPT: `Respond in plain-text only. Keep your responses short and succinct. Ignore any unusual endings like "Bye!" and "Thank you!". Provide a response that is written to be read exactly and literally as it is written. For example, instead of writing '$10-50 million', write '10 to 50 million dollars'. Avoid using symbols or abbreviations that might be misinterpreted when read literally.`
};

const provider = getLLMProvider();
const app = new Elysia();

app.get('/', Bun.file('./public/ui.html'));
app.get('/public/:file', ({ params }) => Bun.file(`./public/${params.file}`));
app.get('/play/:file', ({ params }) => Bun.file(`${CONFIG.TMP}/output/${params.file}`));

app.post('/chat', async ({ body }) => {
	const { id, file, messages: history } = body as any;
	const prevMessages = JSON.parse(history);

	await Bun.write(`${CONFIG.TMP}/input/${id}.wav`, file);
	const transcript = await time(() => transcribe(id), 'Transcription');
	console.log(`${id}:`, transcript);

	if (!transcript) return { messages: prevMessages, error: 'No speech detected!' };

	const messages = [...prevMessages, { role: 'user', content: transcript }];
	const { textStream } = await streamText({
		model: provider,
		system: CONFIG.SYSTEM_PROMPT,
		messages
	});

	const ttsStream = new TTSStream(id);
	const message = await time(() => processTextStream(textStream, ttsStream), 'TextGen & TTS');

	return {
		audio: `/play/${id}.wav`,
		messages: [...messages, { role: 'assistant', content: message }]
	};
});

app.listen(3000, () => {
	open('http://localhost:3000').catch(() => console.log('Open http://localhost:3000 in browser'));
});

function getLLMProvider() {
	const env = process.env;
	if (env.OLLAMA_MODEL) return ollama(env.OLLAMA_MODEL);
	if (env.DEEPSEEK_API_KEY) return deepseek(env.DEEPSEEK_MODEL || 'deepseek-chat');
	if (env.ANTHROPIC_API_KEY) return anthropic(env.ANTHROPIC_MODEL || 'claude-3-5-haiku-20241022');
	if (env.OPENAI_API_KEY) return openai(env.OPENAI_MODEL || 'gpt-4-turbo');
	if (env.TOGETHER_AI_API_KEY)
		return togetherai(env.TOGETHER_AI_MODEL || 'deepseek-ai/DeepSeek-V3');

	console.error(
		'No valid LLM provider found! Set at least one of OLLAMA_MODEL or <PROVIDER>_API_KEY.'
	);
	process.exit(1);
}

function transcribe(id: string): Promise<string> {
	return new Promise((resolve, reject) => {
		const whisperProcess = spawn(CONFIG.WHISPER, [CONFIG.WHISPER_MODEL, id]);
		whisperProcess.stdout.on('end', () => {
			fs.readFile(`${CONFIG.TMP}/transcripts/${id}.txt`, 'utf-8')
				.then((tsc) => resolve(cleanTranscript(tsc)))
				.catch(reject);
		});
	});
}

class TTSStream {
	path: string;
	output: WriteStream;
	kokoroProcess: ChildProcessWithoutNullStreams;

	constructor(id: string) {
		this.path = `${CONFIG.TMP}/output/${id}.wav`;
		this.output = createWriteStream(this.path);
		this.kokoroProcess = spawn(CONFIG.KOKORO, ['--stream'], { stdio: 'pipe' });
		this.kokoroProcess.stdout.pipe(this.output);
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

	for await (const chunk of textStream) {
		buffer += chunk;
		message += chunk;

		const match = buffer.match(/.*?[.!?](\s|$)/);
		if (match) {
			ttsStream.write(match[0].trim());
			buffer = buffer.slice(match[0].length).trim();
		}
	}
	if (buffer) ttsStream.write(buffer.trim());
	await ttsStream.close();
	return message;
}

async function time(fn: Function, label: string) {
	const start = performance.now();
	const result = await fn();
	const duration = Math.round(performance.now() - start);
	console.log(`[${label} took ${duration}ms]`);
	return result;
}

function cleanTranscript(input: string) {
	return input
		.replace(/\[.*?\]/g, '')
		.trim()
		.replace(/\s+/g, ' ');
}
