import React, { useState, useRef } from 'react';

const AudioRecorder = () => {
    const [recording, setRecording] = useState(false);
    const [audioURL, setAudioURL] = useState(null);
    const audioContextRef = useRef(null);
    const mediaRecorderRef = useRef(null);
    const mediaStreamRef = useRef(null);
    const audioChunksRef = useRef([]);

    const startRecording = async () => {
        // Initialize AudioContext for 16kHz
        audioContextRef.current = new (window.AudioContext || window.webkitAudioContext)({ sampleRate: 16000 });

        // Request access to the microphone
        mediaStreamRef.current = await navigator.mediaDevices.getUserMedia({ audio: true });

        // Create MediaRecorder for the audio stream
        mediaRecorderRef.current = new MediaRecorder(mediaStreamRef.current);
        mediaRecorderRef.current.ondataavailable = (event) => {
            if (event.data.size > 0) {
                audioChunksRef.current.push(event.data);
            }
        };

        mediaRecorderRef.current.onstop = async () => {
            // Convert audio chunks to a Blob
            const audioBlob = new Blob(audioChunksRef.current, { type: 'audio/pcm' });
            const arrayBuffer = await audioBlob.arrayBuffer();
            const audioBuffer = await audioContextRef.current.decodeAudioData(arrayBuffer);

            // Downsample, convert to mono and float 32-bit PCM
            const downsampledBuffer = await downsampleBuffer(audioBuffer, 16000);
            const pcmData = convertTo16BitPCM(downsampledBuffer);

            // send pcmData to the server
            const res = await fetch('/api/audio_input', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/octet-stream',
                },
                body: pcmData // ArrayBuffer,
            });

            // Log the response
            const text = await res.text();
            console.log(res.status);
            console.log(text);

            // Create a Blob URL for playback
            //const blob = new Blob([pcmData], { type: 'audio/pcm' });
            //const url = URL.createObjectURL(blob);
            //setAudioURL(url);

            // Reset audio chunks for the next recording
            audioChunksRef.current = [];
        };

        // Start recording
        mediaRecorderRef.current.start();
        setRecording(true);
    };

    const stopRecording = () => {
        mediaRecorderRef.current.stop();
        setRecording(false);

        // Stop media stream
        mediaStreamRef.current.getTracks().forEach(track => track.stop());
    };

    // Downsample the audio buffer to the target sample rate (16kHz)
    const downsampleBuffer = async (buffer, targetSampleRate) => {
        const sampleRate = buffer.sampleRate;
        const sampleRateRatio = sampleRate / targetSampleRate;
        const newLength = Math.round(buffer.length / sampleRateRatio);
        const offlineContext = new OfflineAudioContext(1, newLength, targetSampleRate);
        const source = offlineContext.createBufferSource();
        source.buffer = buffer;
        source.connect(offlineContext.destination);
        source.start(0);
        return await offlineContext.startRendering();
    };


    // Convert audio buffer to 16-bit PCM
    const convertTo16BitPCM = (audioBuffer) => {
        const pcmArray = new Int16Array(audioBuffer.length);

        // Copy and convert to 16-bit integers
        for (let i = 0; i < audioBuffer.length; i++) {
            const s = Math.max(-1, Math.min(1, audioBuffer.getChannelData(0)[i]));
            pcmArray[i] = s < 0 ? s * 0x8000 : s * 0x7FFF;
        }
        console.log(pcmArray);
        return pcmArray;
    };

    const convertToPCM = (audioBuffer) => {

        const pcmArray = new Float32Array(audioBuffer.length);

        // Copy channel data (assuming mono)
        audioBuffer.copyFromChannel(pcmArray, 0);
        console.log(pcmArray);
        return pcmArray;
    };

    return (
        <div>
            <button onClick={recording ? stopRecording : startRecording}>
                {recording ? 'Stop Recording' : 'Start Recording'}
            </button>
            {audioURL && <audio controls src={audioURL} />}
        </div>
    );
};

export default AudioRecorder;
