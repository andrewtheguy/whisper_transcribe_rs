import { useState, useEffect, useCallback, useRef } from 'react';

import atanProcessorUrl from "./audioProcessor.js?url";


const AudioStreamComponent = () => {
  const [isRecording, setIsRecording] = useState(false);
  //const [audioContext, setAudioContext] = useState(null);
  //const [workletNode, setWorkletNode] = useState(null);
  //const [mediaStream, setMediaStream] = useState(null);


  const [count, setCount] = useState(0);
  const audioContextRef = useRef(null);
  const workletNode = useRef(null);
  const mediaStreamRef = useRef(null);
  const bufferRef = useRef(new Uint8Array());

  const timerRef = useRef(null);

  const callweb = useCallback(async (int16Data) => {
    
        // send pcmData to the server
        const res = await fetch('/api/audio_input', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/octet-stream',
            },
            body: int16Data // ArrayBuffer,
        });

        // Log the response
        const text = await res.text();
        console.log(res.status);
        console.log(text);
}, []);


  const processfunc = useCallback(() => {


        if (bufferRef.current.length > 16000) {
            const int16Data = bufferRef.current;
            bufferRef.current = new Uint8Array();
            callweb(int16Data);
        }


        setTimeout(() => {
            processfunc();
          }, 1000); // delay of 1 second


   }, []);

   useEffect(() => {

    processfunc();
 
    
   }, []);

  const initializeAudio = useCallback(async () => {
   
      const audioContext = new (window.AudioContext || window.webkitAudioContext)({ sampleRate: 16000 });
      
      // Load the audio worklet
      await audioContext.audioWorklet.addModule(atanProcessorUrl);
      
      const stream = await navigator.mediaDevices.getUserMedia({ audio: true });

 
      // Create audio worklet node with exactly 1 input and 1 output channel
      const worklet = new AudioWorkletNode(audioContext, 'audio-processor', {
        numberOfInputs: 1,
        numberOfOutputs: 1,
        channelCount: 1,
        channelCountMode: 'explicit',
        channelInterpretation: 'speakers'
      });
      
      
      // Handle PCM data from the worklet
      worklet.port.onmessage = (event) => {
        if (event.data.type === 'pcm') {
            //const pcmData = event.data.data;
            //console.log('Int16 PCM Data:', event.data.int16Data);

            const pcmData = event.data.rawBytes;

            bufferRef.current = Uint8Array.from([...bufferRef.current, ...pcmData])


          //console.log('int16Data Data:', int16Data);
        }
      };
      

    audioContextRef.current = audioContext;

    // Request access to the microphone
    workletNode.current = worklet;

    mediaStreamRef.current = stream;

 
    const source = audioContext.createMediaStreamSource(stream);
    source.connect(worklet);
    worklet.connect(audioContext.destination);
    
  }, []);

  const startRecording = useCallback(async () => {
    await initializeAudio();
     
      setIsRecording(true);

  },[]);

  const stopRecording = useCallback(() => {
    
    audioContextRef.current.close();
    
    workletNode.current.disconnect();

     // Stop media stream
     mediaStreamRef.current.getTracks().forEach(track => track.stop());

     const int16Data = bufferRef.current;
     bufferRef.current = new Uint8Array();
     callweb(int16Data);

     setIsRecording(false);
  }, []);

 
  return (
    <div className="p-4">
      <button
        onClick={isRecording ? stopRecording : startRecording}
        className="px-4 py-2 bg-blue-500 text-white rounded hover:bg-blue-600 focus:outline-none"
      >
        {isRecording ? 'Stop Recording' : 'Start Recording'}
      </button>
    </div>
  );
};

export default AudioStreamComponent;