//import reactLogo from './assets/react.svg'
//import viteLogo from '/vite.svg'
import './App.css'
import AudioStreamComponent from './AudioStreamComponent'
import { useEffect, useState } from 'react';
import ChatComponent from './ChatComponent';

function App() {
  const [transcripts, setTranscripts] = useState<JSX.Element[]>([]);
  const [afterId, setAfterId] = useState(0);
  const [counter, setCounter] = useState(0);
  const [showName, setShowName] = useState<string | null>(null);
  const [showNameList, setShowNameList] = useState<string[] | null>(null);

  async function fetchTranscripts() {
    if(showName === null){
      return;
    }
    const res = await fetch('/api/get_transcripts?' + new URLSearchParams({
  show_name: showName,
  after_id: afterId.toString(),
  }));
    const data = await res.text();
    const lines = data.split(/\n/);
    let new_after_id = null;
    const arr = lines.reduce<JSX.Element[]>((accumulator, line) => {
      //console.log(`line: ${line.length}`);
      if (line.length === 0) {
        return accumulator;
      }
      const object = JSON.parse(line);
      accumulator.push(
        <div key={object.id} style={{textAlign: 'left'}}>
          <b>{object.timestamp}: </b>
          <span>{object.content}</span>
        </div>
      );
      new_after_id = object.id;
      return accumulator;
    },[]);
    //setTranscripts(transcripts.concat(arr).slice(-100));
    setTranscripts(transcripts.concat(arr));

    if (new_after_id !== null) {
      setAfterId(new_after_id);
    }else{
      setCounter(counter + 1);
    }
    //setTimeout(fetchTranscripts, 500);
  }

  useEffect(() => {
    if(showName !== null){
      console.log('afterId: ' + afterId);
      setTimeout(fetchTranscripts, 500);
    }
  },[afterId,counter,showName]);

  useEffect(() => {
    const element = document.getElementById('transcripts');
    if (element) {
      element.scrollTop = element?.scrollHeight;
    }
  },[transcripts]);

  useEffect(() => {
    
    setTranscripts([]);
    setAfterId(0);
    
  }, [showName]);

  useEffect(() => {
    (async () => {
      const res = await fetch('/api/get_show_names');
      const showNames = await res.json();
      setShowNameList(showNames);
    })();
  }, []);

  if(showNameList === null){
    return <div>Loading...</div>;
  }

  return (
    <>
      <h1>Vite + React</h1>
      <div className="">
        <AudioStreamComponent />
        <p>
          start recording will invalidate other sessions
        </p>
      </div>
      <h2>Transcripts</h2>
      <select onChange={(e) => setShowName(e.target.value)}>
        <option value="">Select a show</option>
        {showNameList?.map((name) => (
          <option key={name} value={name}>{name}</option>
        ))}
      </select>
      <ChatComponent messages={transcripts} />
    </>
  )
}

export default App
