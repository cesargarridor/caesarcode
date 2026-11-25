import ReactDOM from 'react-dom/client';
import '@fontsource-variable/inter';
import '@fontsource/jetbrains-mono/400.css';
import '@fontsource/jetbrains-mono/500.css';
import '@fontsource/jetbrains-mono/700.css';
import './index.css';
import './store/settings'; // aplica tema y tipografía guardados antes del primer render
import App from './App';

// Sin React.StrictMode: evita doble suscripción a Channels / doble PTY en dev.
ReactDOM.createRoot(document.getElementById('root')!).render(<App />);
