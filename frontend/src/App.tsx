//import reactLogo from './assets/react.svg'
//import viteLogo from '/vite.svg'
import './App.css'
import AudioStreamComponent from './AudioStreamComponent'
import { useEffect, useReducer, useRef, useState } from 'react';
import ChatComponent from './ChatComponent';

interface Transcript {
  id: number;
  timestamp: string;
  content: string;
}

interface MessageBox {
  transcripts: Transcript[];
  afterId: number;
}

interface MessageAction {
  type: string;
  payload?: Transcript[];
  newAfterId?: number;
}

const messageReducer = (state: MessageBox, action: MessageAction) => {
  let newState = {...state};
  switch (action.type) {
    case "ADD_ITEMS_END":
      //console.log('ADD_ITEMS_END action.payload', action.payload);
      newState.transcripts = [...state.transcripts,...action.payload || []];
      if (action.newAfterId !== undefined) {
        newState.afterId = action.newAfterId;
      }
      break;
    case "ADD_ITEMS_START":
      //console.log('ADD_ITEMS_START action.payload', action.payload);
      newState.transcripts = [...action.payload || [],...state.transcripts];
      break;
    case "CLEAR":
      newState.transcripts = [];
      newState.afterId = 0;
      break;
    //case "SET_AFTER_ID":
    //  console.log('SET_AFTER_ID action.payload', action.newAfterId);
    //  newState.afterId = action.newAfterId || 0;
    //  break;
    default:
      break;
  }
  //console.log('newState', newState);
  return newState;
};


function App() {
  
  const [messageBox, dispatch] = useReducer(messageReducer, {transcripts: [], afterId: 0});
  
  //const [transcripts, setTranscripts] = useState<Transcript[]>([]);
  //const [afterId, setAfterId] = useState(0);
  const [counter, setCounter] = useState(0);
  const [showName, setShowName] = useState<string | null>(null);
  const [showNameList, setShowNameList] = useState<string[] | null>(null);
  const [pauseFetchAfter, setPauseFetchAfter] = useState(false);
  const timerRef = useRef<Number | null>(null);
  
  async function fetchTranscripts(){
    if(showName === null){
      return;
    }
    if(pauseFetchAfter){
      return;
    }
    const {afterId} = messageBox;
    const res = await fetch('/api/get_transcripts?' + new URLSearchParams({
      show_name: showName,
      after_id: afterId.toString(),
    }));
    const data = await res.text();
    const lines = data.split(/\n/);
    let newAfterId = undefined;
    const arr = lines.reduce<Transcript[]>((accumulator, line) => {
      //console.log(`line: ${line.length}`);
      if (line.length === 0) {
        return accumulator;
      }
      const object = JSON.parse(line);
      accumulator.push({id: object.id, timestamp: object.timestamp, content: object.content});
      newAfterId = object.id;
      return accumulator;
    },[]);
    //debugger;
    //setTranscripts(transcripts.concat(arr).slice(-100));
    dispatch({ type: "ADD_ITEMS_END", payload: arr, newAfterId: newAfterId });

    setCounter(counter + 1);
    
    //setTimeout(fetchTranscripts, 500);
  }
  
  
  async function fetchPrevTranscripts(){
    const {transcripts} = messageBox;
    if(showName === null){
      return;
    }
    if(transcripts.length === 0){
      return;
    }
    setPauseFetchAfter(true);
    try{
      const res = await fetch('/api/get_transcripts?' + new URLSearchParams({
        show_name: showName,
        before_id: transcripts[0].id.toString(),
      }));
      const data = await res.text();
      const lines = data.split(/\n/);
      
      const arr = lines.reduce<Transcript[]>((accumulator, line) => {
        //console.log(`line: ${line.length}`);
        if (line.length === 0) {
          return accumulator;
        }
        const object = JSON.parse(line);
        accumulator.push({id: object.id, timestamp: object.timestamp, content: object.content});
        return accumulator;
      },[]);
      console.log('arr', arr);
      //setTranscripts(transcripts.concat(arr).slice(-100));
      dispatch({ type: "ADD_ITEMS_START", payload: arr });
    }finally{
      setPauseFetchAfter(false);
    }
    //setTimeout(fetchTranscripts, 500);
  }
  
  useEffect(() => {
    //const {transcripts, afterId} = messageBox;
    if(showName !== null){
      //console.log('afterId: ' + afterId);
      if(!pauseFetchAfter){
        timerRef.current=setTimeout(fetchTranscripts, 500);
      }
    }

    return () => {
      if(timerRef.current){
        clearTimeout(timerRef.current as number);
      }
    };
  },[counter,showName,pauseFetchAfter]);
  
  useEffect(() => {
    const element = document.getElementById('transcripts');
    if (element) {
      element.scrollTop = element?.scrollHeight;
    }
  },[messageBox.transcripts]);
  
  useEffect(() => {
    
    //setTranscripts([]);
    dispatch({ type: "CLEAR", });
    //setAfterId(0);
    
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

  const {transcripts} = messageBox;
  
  const transcriptList = transcripts.map((transcript) => (
    <div key={transcript.id} style={{textAlign: "left"}}>
    <span>{transcript.timestamp}</span>
    <span>{transcript.content}</span>
    </div>
  ));
  
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
    <button onClick={() => fetchPrevTranscripts()}>Load Prev</button>
    {/* <button onClick={() => fetchTranscripts()}>Load More</button> */}
    <ChatComponent messages={transcriptList} />
    </>
  )
}

export default App;
