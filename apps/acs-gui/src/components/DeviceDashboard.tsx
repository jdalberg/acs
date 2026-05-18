import { useState, useEffect } from 'react';
import { ArrowLeft, Cpu, HardDrive, Clock, Box, ShieldCheck } from 'lucide-react';
import type { Device } from '../App';
import ObjectBrowser from './ObjectBrowser';

interface DeviceDashboardProps {
  uid: string;
  onBack: () => void;
}

export default function DeviceDashboard({ uid, onBack }: DeviceDashboardProps) {
  const [device, setDevice] = useState<Device | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<'overview' | 'browser'>('overview');

  useEffect(() => {
    fetch(`http://localhost:8080/api/v1/inventory/devices/${uid}`)
      .then(res => {
        if (!res.ok) throw new Error('Failed to fetch device details');
        return res.json();
      })
      .then(data => {
        setDevice(data);
        setLoading(false);
      })
      .catch(err => {
        setError(err.message);
        setLoading(false);
      });
  }, [uid]);

  if (loading) {
    return (
      <div className="glass-panel" style={{ padding: '3rem', textAlign: 'center' }}>
        <p>Loading device data...</p>
      </div>
    );
  }

  if (error || !device) {
    return (
      <div className="glass-panel" style={{ padding: '2rem', borderColor: 'var(--danger-color)' }}>
        <button className="back-button" onClick={onBack} style={{ marginBottom: '1rem' }}>
          <ArrowLeft size={20} />
        </button>
        <h2 style={{ color: 'var(--danger-color)' }}>Error Loading Device</h2>
        <p>{error || 'Device not found'}</p>
      </div>
    );
  }

  return (
    <div className="dashboard-container">
      <div className="dashboard-header">
        <button className="back-button" onClick={onBack} title="Back to Search">
          <ArrowLeft size={20} />
        </button>
        <div>
          <h2 style={{ fontSize: '1.5rem', fontWeight: '600' }}>{device.serial_number}</h2>
          <div style={{ color: 'var(--text-secondary)', fontSize: '0.875rem' }}>
            {device.manufacturer} • {device.oui}
          </div>
        </div>
      </div>

      <div className="glass-panel" style={{ padding: '1.5rem', marginBottom: '1.5rem' }}>
        <div className="tabs">
          <button 
            className={`tab ${activeTab === 'overview' ? 'active' : ''}`}
            onClick={() => setActiveTab('overview')}
          >
            Overview
          </button>
          <button 
            className={`tab ${activeTab === 'browser' ? 'active' : ''}`}
            onClick={() => setActiveTab('browser')}
          >
            Object Browser
          </button>
        </div>

        {activeTab === 'overview' && (
          <div className="info-grid" style={{ padding: '1rem 0' }}>
            <div className="info-item">
              <span className="info-label"><Cpu size={14} style={{ display: 'inline', marginRight: '4px' }} /> Hardware Version</span>
              <span className="info-value">{device.hardware_version || 'Unknown'}</span>
            </div>
            <div className="info-item">
              <span className="info-label"><Box size={14} style={{ display: 'inline', marginRight: '4px' }} /> Software Version</span>
              <span className="info-value">{device.software_version || 'Unknown'}</span>
            </div>
            <div className="info-item">
              <span className="info-label"><ShieldCheck size={14} style={{ display: 'inline', marginRight: '4px' }} /> Product Class</span>
              <span className="info-value">{device.product_class || 'Unknown'}</span>
            </div>
            <div className="info-item">
              <span className="info-label"><Clock size={14} style={{ display: 'inline', marginRight: '4px' }} /> Last Seen</span>
              <span className="info-value">
                {device.last_seen ? new Date(device.last_seen).toLocaleString() : 'Never'}
              </span>
            </div>
            <div className="info-item">
              <span className="info-label"><HardDrive size={14} style={{ display: 'inline', marginRight: '4px' }} /> Protocol</span>
              <span className="info-value" style={{ textTransform: 'uppercase' }}>{device.current_protocol || 'CWMP'}</span>
            </div>
          </div>
        )}

        {activeTab === 'browser' && (
          <ObjectBrowser uid={uid} />
        )}
      </div>
    </div>
  );
}
