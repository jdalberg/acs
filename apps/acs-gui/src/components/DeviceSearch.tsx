import { useState, useEffect } from 'react';
import { Search, Router, Wifi, WifiOff } from 'lucide-react';
import type { Device } from '../App';

interface DeviceSearchProps {
  onSelectDevice: (uid: string) => void;
}

export default function DeviceSearch({ onSelectDevice }: DeviceSearchProps) {
  const [devices, setDevices] = useState<Device[]>([]);
  const [searchTerm, setSearchTerm] = useState('');
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    fetch('http://localhost:8080/api/v1/inventory/devices')
      .then(res => {
        if (!res.ok) throw new Error('Failed to fetch devices');
        return res.json();
      })
      .then(data => {
        setDevices(data);
        setLoading(false);
      })
      .catch(err => {
        setError(err.message);
        setLoading(false);
      });
  }, []);

  const filteredDevices = devices.filter(d => 
    d.serial_number.toLowerCase().includes(searchTerm.toLowerCase()) ||
    d.oui.toLowerCase().includes(searchTerm.toLowerCase()) ||
    d.manufacturer.toLowerCase().includes(searchTerm.toLowerCase())
  );

  const isOnline = (lastSeen?: string) => {
    if (!lastSeen) return false;
    // Simple heuristic: if seen in the last 5 minutes, consider online
    const diff = new Date().getTime() - new Date(lastSeen).getTime();
    return diff < 5 * 60 * 1000;
  };

  if (loading) {
    return (
      <div className="glass-panel" style={{ padding: '3rem', textAlign: 'center' }}>
        <div className="loader spin"><Router size={32} /></div>
        <p style={{ color: 'var(--text-secondary)' }}>Discovering devices...</p>
      </div>
    );
  }

  if (error) {
    return (
      <div className="glass-panel" style={{ padding: '2rem', borderColor: 'var(--danger-color)' }}>
        <h2 style={{ color: 'var(--danger-color)', marginBottom: '1rem' }}>Connection Error</h2>
        <p>{error}</p>
      </div>
    );
  }

  return (
    <div className="glass-panel" style={{ padding: '2rem' }}>
      <div style={{ position: 'relative', marginBottom: '2rem' }}>
        <div style={{ position: 'absolute', left: '1rem', top: '50%', transform: 'translateY(-50%)', color: 'var(--text-secondary)' }}>
          <Search size={20} />
        </div>
        <input 
          type="text" 
          placeholder="Search by serial number, OUI, or manufacturer..." 
          value={searchTerm}
          onChange={e => setSearchTerm(e.target.value)}
          style={{ paddingLeft: '3rem' }}
        />
      </div>

      {filteredDevices.length === 0 ? (
        <div style={{ textAlign: 'center', padding: '3rem', color: 'var(--text-secondary)' }}>
          <p>No devices found matching your search.</p>
        </div>
      ) : (
        <div className="device-grid">
          {filteredDevices.map(device => {
            const online = isOnline(device.last_seen);
            return (
              <div 
                key={device.device_uid} 
                className="glass-panel device-card"
                onClick={() => onSelectDevice(device.device_uid)}
              >
                <div className="device-card-header">
                  <div style={{ display: 'flex', alignItems: 'center', gap: '0.75rem' }}>
                    <Router size={24} color="var(--accent-color)" />
                    <div>
                      <div className="device-title">{device.serial_number}</div>
                      <div className="device-subtitle">{device.manufacturer}</div>
                    </div>
                  </div>
                  <div className={`status-badge ${online ? '' : 'offline'}`} title={device.last_seen ? new Date(device.last_seen).toLocaleString() : 'Never seen'}>
                    {online ? (
                      <span style={{ display: 'flex', alignItems: 'center', gap: '4px' }}><Wifi size={12} /> Online</span>
                    ) : (
                      <span style={{ display: 'flex', alignItems: 'center', gap: '4px' }}><WifiOff size={12} /> Offline</span>
                    )}
                  </div>
                </div>
                
                <div style={{ display: 'flex', gap: '1rem', marginTop: '1.5rem', fontSize: '0.875rem', color: 'var(--text-secondary)' }}>
                  <div>
                    <span style={{ display: 'block', fontSize: '0.75rem', textTransform: 'uppercase', marginBottom: '2px' }}>OUI</span>
                    <span style={{ color: 'var(--text-primary)' }}>{device.oui}</span>
                  </div>
                  {device.software_version && (
                    <div>
                      <span style={{ display: 'block', fontSize: '0.75rem', textTransform: 'uppercase', marginBottom: '2px' }}>Firmware</span>
                      <span style={{ color: 'var(--text-primary)' }}>{device.software_version}</span>
                    </div>
                  )}
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
