import { useState } from 'react';
import { Activity } from 'lucide-react';
import DeviceSearch from './components/DeviceSearch';
import DeviceDashboard from './components/DeviceDashboard';
import './index.css';

export interface Device {
  device_uid: string;
  manufacturer: string;
  oui: string;
  serial_number: string;
  software_version?: string;
  hardware_version?: string;
  product_class?: string;
  current_protocol?: string;
  last_seen?: string;
}

function App() {
  const [selectedDeviceUid, setSelectedDeviceUid] = useState<string | null>(null);

  return (
    <div className="app-container">
      <header className="header">
        <div style={{ background: 'rgba(59, 130, 246, 0.2)', padding: '0.75rem', borderRadius: '12px' }}>
          <Activity size={28} color="#60a5fa" />
        </div>
        <h1>ACS Reactivity Center</h1>
      </header>

      <main>
        {!selectedDeviceUid ? (
          <DeviceSearch onSelectDevice={setSelectedDeviceUid} />
        ) : (
          <DeviceDashboard 
            uid={selectedDeviceUid} 
            onBack={() => setSelectedDeviceUid(null)} 
          />
        )}
      </main>
    </div>
  );
}

export default App;
