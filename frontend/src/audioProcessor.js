// audioProcessor.js
class AudioProcessor extends AudioWorkletProcessor {
    constructor() {
      super();
      this.isRecording = false;
    }
  
    float32ToInt16(float32Array) {
      const int16Array = new Int16Array(float32Array.length);
      for (let i = 0; i < float32Array.length; i++) {
        // Convert float32 value (-1 to 1) to int16 (-32768 to 32767)
        const s = Math.max(-1, Math.min(1, float32Array[i]));
        int16Array[i] = s < 0 ? s * 0x8000 : s * 0x7FFF;
      }
      return int16Array;
    }

    float32ToInt16Bytes(float32Array) {
        // First convert float32 to int16
        const int16Array = new Int16Array(float32Array.length);
        for (let i = 0; i < float32Array.length; i++) {
          const s = Math.max(-1, Math.min(1, float32Array[i]));
          int16Array[i] = s < 0 ? s * 0x8000 : s * 0x7FFF;
        }
        
        // Convert Int16Array to Uint8Array (raw bytes)
        const uint8Array = new Uint8Array(int16Array.buffer);
        return uint8Array;
      }
  
    process(inputs, outputs, parameters) {
      const input = inputs[0];
      if (input && input.length > 0) {
        // Ensure mono by using only the first channel
        const monoData = input[0];
        //const int16Data = this.float32ToInt16(monoData);
        const rawBytes = this.float32ToInt16Bytes(monoData);
        
        this.port.postMessage({ 
          type: 'pcm',
          float32Data: Array.from(monoData),  // Original mono data
          rawBytes: rawBytes, // Raw bytes 16 bit int
        });
      }
      return true;
    }
  }
  
  registerProcessor('audio-processor', AudioProcessor);