import { useEffect, useRef, useState } from "react";

interface ChatComponentProps {
    messages: JSX.Element[];
  }

const ChatComponent = ( { messages } : ChatComponentProps) => {
  const chatEndRef = useRef<HTMLDivElement | null>(null);  // Reference to the end of the chat content
  const containerRef = useRef<HTMLDivElement | null>(null); // Reference to the chat container
  const [userScrolledUp, setUserScrolledUp] = useState(false);

  const scrollToBottom = () => {
    chatEndRef.current?.scrollIntoView({ behavior: "instant" });
  };

  // Set userScrolledUp based on scroll position
  const handleScroll = () => {
    if (!containerRef.current) return;
    
    // Check if the user has scrolled up (not at the bottom)
    const isAtBottom = containerRef.current.scrollHeight - containerRef.current.scrollTop === containerRef.current.clientHeight;
    
    setUserScrolledUp(!isAtBottom);
  };

  // Listen for scroll event to detect if the user scrolled up
  useEffect(() => {
    const container = containerRef.current;
    container?.addEventListener("scroll", handleScroll);
    
    return () => container?.removeEventListener("scroll", handleScroll);
  }, []);

  // Scroll to the bottom only when the component mounts or messages change
  useEffect(() => {
    if (!userScrolledUp) {
      scrollToBottom();
    }
  }, [messages, userScrolledUp]);

  return (
    <div ref={containerRef} className="chat-box">
      {messages}
      <div ref={chatEndRef} />
    </div>
  );
};

export default ChatComponent;
