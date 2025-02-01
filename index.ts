import { Elysia } from 'elysia';
import { streamText } from 'ai';
import { createTogetherAI } from '@ai-sdk/togetherai';
import { spawn } from 'node:child_process';
import fs from 'fs/promises';
import { createWriteStream, type WriteStream } from 'fs';

const KOKORO = './kokoro/target/release/koko';
const WHISPER = './whisper/target/release/whisper';
const TMP = './tmp';

const togetherai = createTogetherAI({
	apiKey: process.env.TOGETHER_API_KEY
});

const app = new Elysia();

app.get('/', Bun.file('./public/ui.html'));
app.get('/public/:filename', ({ params }) => Bun.file(`./public/${params.filename}`));
app.get('/play/:filename', ({ params }) => Bun.file(`./tmp/output/${params.filename}`));

app.post('/chat', async ({ body }) => {
	const { id, file, messages: history } = body as any;

	await Bun.write(`${TMP}/input/${id}.wav`, file);

	const a = performance.now();
	const transcript = await transcribe(id);
	console.log(performance.now() - a);
	console.log('Transcript: ', transcript);

	const prompt = { role: 'user', content: transcript };

	const messages = [...JSON.parse(history), prompt];

	const { textStream } = await streamText({
		model: togetherai('deepseek-ai/DeepSeek-V3'),
		system: 'Respond with plain-text responses only. Keep your responses short and succint.',
		messages
	});

	const ttsStream = new TTSStream(id);
	const message = await processTextStream(textStream, ttsStream);

	console.log('Response: ', message);

	return {
		answer: `/play/${id}.wav`,
		messages: [...messages, { role: 'assistant', content: message }]
	};
});

app.listen(3000, () => console.log('Listening at http://localhost:3000'));

function transcribe(id: string): Promise<string> {
	const whisperProcess = spawn(WHISPER, [id]);

	return new Promise((resolve, reject) => {
		whisperProcess.stdout.on('end', () => {
			fs.readFile(`${TMP}/transcripts/${id}.txt`)
				.then((buffer) => resolve(buffer.toString().trim()))
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

		this.kokoroProcess.stdout.on('data', (chunk) => {
			this.output.write(chunk);
		});

		this.kokoroProcess.stdout.on('end', () => {
			this.output.end();
		});
	}

	write(input: string) {
		this.kokoroProcess.stdin.write(input + '\n');
	}

	close() {
		this.kokoroProcess.stdin.end();
	}
}

async function processTextStream(textStream: AsyncIterable<string>, ttsStream: TTSStream) {
	let buffer = '';
	let message = '';

	for await (const textChunk of textStream) {
		buffer += textChunk;
		message += textChunk;

		const sentenceEndRegex = /[.!?]\s/;
		const sentenceEndIdx = buffer.search(sentenceEndRegex);
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
	ttsStream.close();

	return message;
}
