import fs from 'fs';
import { spawn } from 'node:child_process';
import { v4 as uuidv4 } from 'uuid';

const kokoPath = './kokoro/target/release/koko';

export class TTSStream {
	id = uuidv4();
	path = `output/${this.id}.wav`;
	output = fs.createWriteStream(this.path);
	kokoProcess = spawn(kokoPath, ['--stream'], { stdio: 'pipe' });
	// rawAudioData = Buffer.alloc(0);

	constructor() {
		this.kokoProcess.stdout.on('data', (chunk) => {
			// rawAudioData = Buffer.concat([rawAudioData, chunk]);
			this.output.write(chunk);
		});

		// this.kokoProcess.stdin.write(
		// 	`Hello, This is Kokoro, your remarkable AI TTS. It's a TTS model with merely 82 million parameters yet delivers incredible audio quality.
		//                 This is one of the top notch Rust based inference models, and I'm sure you'll love it. If you do, please give us a star. Thank you very much.
		//                 As the night falls, I wish you all a peaceful and restful sleep. May your dreams be filled with joy and happiness. Good night, and sweet dreams!` +
		// 		'\n'
		// );
		// this.kokoProcess.stdin.end();

		this.kokoProcess.stdout.on('end', () => {
			// fs.writeFileSync('output.wav', rawAudioData);
			this.output.end();
		});
	}

	write(input: string) {
		this.kokoProcess.stdin.write(input + '\n');
	}

	close() {
		this.kokoProcess.stdin.end();
	}
}

// new TTSStream();

// export function generateSpeech() {}

// const inputText = 'This is the text to synthesize into audio';

// const kokoProcess = spawn(kokoPath, ['--stream'], { stdio: 'pipe' });
// kokoProcess.stdin.write(inputText + '\n');
// // kokoProcess.stdin.end();

// let rawAudioData = Buffer.alloc(0);

// kokoProcess.stdout.on('data', (chunk) => {
// 	rawAudioData = Buffer.concat([rawAudioData, chunk]);
// });

// kokoProcess.stdout.on('end', () => {
// 	console.log('Finished receiving audio data.');

// 	fs.writeFileSync('output.raw', rawAudioData);
// });

// kokoProcess.stderr.on('data', (data) => {
// 	console.error(`koko stderr: ${data}`);
// });

// kokoProcess.on('close', (code) => {
// 	console.log(`koko process exited with code ${code}`);
// });

// let lineCount = 0;
// const intervalId = setInterval(() => {
// 	lineCount++;
// 	kokoProcess.stdin.write(`This is line ${lineCount}\n`);

// 	if (lineCount === 5) {
// 		// Stop sending text after 5 lines
// 		clearInterval(intervalId);
// 		kokoProcess.stdin.end();
// 	}
// }, 2000);
