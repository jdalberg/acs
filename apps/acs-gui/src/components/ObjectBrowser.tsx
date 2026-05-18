import { useState, useEffect } from 'react';
import { Folder, FileText, ChevronRight, ChevronDown, RefreshCw, Edit3, Check, X } from 'lucide-react';

interface ObjectBrowserProps {
  uid: string;
}

interface ParamNode {
  name: string;
  fullPath: string;
  isObject: boolean;
  writable: boolean;
  value?: string;
  children?: Record<string, ParamNode>;
  expanded?: boolean;
}

export default function ObjectBrowser({ uid }: ObjectBrowserProps) {
  const [tree, setTree] = useState<Record<string, ParamNode>>({});
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Edit State
  const [editingPath, setEditingPath] = useState<string | null>(null);
  const [editValue, setEditValue] = useState<string>('');
  const [saving, setSaving] = useState(false);

  // Fetch a level of parameters
  const fetchLevel = async (pathPrefix: string) => {
    setLoading(true);
    setError(null);
    try {
      const response = await fetch(`http://localhost:8080/api/v1/device/${uid}/command`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          Action: {
            GetParameterNames: {
              path_prefix: pathPrefix,
              next_level: true
            }
          }
        })
      });

      if (!response.ok) {
        const errText = await response.text();
        throw new Error(`Command failed: ${response.status} - ${errText}`);
      }

      const data = await response.json();
      
      if (data.result && data.result.Success) {
        // Build new nodes
        const newNodes: Record<string, ParamNode> = {};
        for (const [path, writableStr] of Object.entries(data.result.Success)) {
          // Skip the prefix itself if it's returned
          if (path === pathPrefix) continue;
          
          const isObject = path.endsWith('.');
          const name = isObject 
            ? path.slice(pathPrefix.length, -1) 
            : path.slice(pathPrefix.length);

          newNodes[name] = {
            name,
            fullPath: path,
            isObject,
            writable: writableStr === 'true',
            children: isObject ? {} : undefined,
            expanded: false
          };
        }

        // Insert into tree
        setTree(prev => {
          const newTree = { ...prev };
          if (pathPrefix === '') {
            return newNodes;
          } else {
            // Find the parent node and attach children
            const attach = (currentLevel: Record<string, ParamNode>, pathParts: string[]) => {
              if (pathParts.length === 0) return;
              const part = pathParts[0];
              if (pathParts.length === 1 && currentLevel[part]) {
                currentLevel[part].children = newNodes;
                currentLevel[part].expanded = true;
              } else if (currentLevel[part] && currentLevel[part].children) {
                attach(currentLevel[part].children!, pathParts.slice(1));
              }
            };
            const parts = pathPrefix.split('.').filter(p => p.length > 0);
            attach(newTree, parts);
            return newTree;
          }
        });
      } else if (data.result && data.result.Fault) {
        throw new Error(`Device Fault: ${data.result.Fault.string} (Code: ${data.result.Fault.code})`);
      } else {
        throw new Error("Unexpected response format");
      }
    } catch (err: any) {
      setError(err.message);
    } finally {
      setLoading(false);
    }
  };

  const fetchValue = async (node: ParamNode) => {
    setLoading(true);
    try {
      const response = await fetch(`http://localhost:8080/api/v1/device/${uid}/command`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          Action: {
            GetParameterValues: {
              paths: [node.fullPath]
            }
          }
        })
      });

      if (!response.ok) throw new Error('Failed to fetch value');
      const data = await response.json();

      if (data.result && data.result.Success && data.result.Success[node.fullPath] !== undefined) {
        updateNodeValue(node.fullPath, data.result.Success[node.fullPath]);
      }
    } catch (err: any) {
      setError(`Value fetch failed: ${err.message}`);
    } finally {
      setLoading(false);
    }
  };

  const saveValue = async (node: ParamNode, newValue: string) => {
    setSaving(true);
    try {
      const response = await fetch(`http://localhost:8080/api/v1/device/${uid}/command`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          Action: {
            SetParameterValues: {
              parameters: { [node.fullPath]: newValue }
            }
          }
        })
      });

      if (!response.ok) throw new Error('Set value failed');
      const data = await response.json();
      
      if (data.result && data.result.Fault) {
        throw new Error(`Device Fault: ${data.result.Fault.string}`);
      }

      // If successful, update the UI value
      updateNodeValue(node.fullPath, newValue);
      setEditingPath(null);
    } catch (err: any) {
      setError(`Save failed: ${err.message}`);
    } finally {
      setSaving(false);
    }
  };

  const updateNodeValue = (fullPath: string, val: string) => {
    setTree(prev => {
      const newTree = { ...prev };
      const parts = fullPath.split('.').filter(p => p.length > 0);
      let current = newTree;
      for (let i = 0; i < parts.length - 1; i++) {
        if (current[parts[i]] && current[parts[i]].children) {
          current = current[parts[i]].children!;
        }
      }
      const lastPart = parts[parts.length - 1];
      if (current[lastPart]) {
        current[lastPart].value = val;
      }
      return newTree;
    });
  };

  const toggleExpand = (node: ParamNode) => {
    if (!node.expanded && (!node.children || Object.keys(node.children).length === 0)) {
      fetchLevel(node.fullPath);
    } else {
      // Toggle locally
      setTree(prev => {
        const newTree = { ...prev };
        const parts = node.fullPath.split('.').filter(p => p.length > 0);
        let current = newTree;
        for (let i = 0; i < parts.length - 1; i++) {
          current = current[parts[i]].children!;
        }
        const lastPart = parts[parts.length - 1];
        current[lastPart].expanded = !current[lastPart].expanded;
        return newTree;
      });
    }
  };

  useEffect(() => {
    // Initial fetch of root objects (e.g. Device., InternetGatewayDevice.)
    fetchLevel('');
  }, []);

  const renderTree = (nodes: Record<string, ParamNode>) => {
    return Object.values(nodes).sort((a, b) => a.name.localeCompare(b.name)).map(node => (
      <div key={node.fullPath}>
        <div className="tree-item" onClick={() => node.isObject ? toggleExpand(node) : fetchValue(node)}>
          {node.isObject ? (
            <>
              {node.expanded ? <ChevronDown size={16} /> : <ChevronRight size={16} />}
              <Folder size={16} color="var(--accent-color)" />
              <span className="tree-label">{node.name}</span>
            </>
          ) : (
            <>
              <div style={{ width: '16px' }} /> {/* indent for alignment */}
              <FileText size={16} color="var(--text-secondary)" />
              <span className="tree-label">{node.name}</span>
              
              {node.value !== undefined && editingPath !== node.fullPath && (
                <span className="tree-value">
                  {node.value || '""'}
                  {node.writable && (
                    <button 
                      className="btn-icon" 
                      style={{ padding: '0 4px', marginLeft: '8px' }}
                      onClick={(e) => { e.stopPropagation(); setEditValue(node.value || ''); setEditingPath(node.fullPath); }}
                    >
                      <Edit3 size={12} />
                    </button>
                  )}
                </span>
              )}

              {editingPath === node.fullPath && (
                <div style={{ marginLeft: 'auto', display: 'flex', alignItems: 'center', gap: '4px' }} onClick={e => e.stopPropagation()}>
                  <input 
                    type="text" 
                    value={editValue} 
                    onChange={e => setEditValue(e.target.value)}
                    style={{ padding: '4px 8px', fontSize: '0.875rem', width: '200px' }}
                    autoFocus
                  />
                  <button className="btn-icon" onClick={() => saveValue(node, editValue)} disabled={saving}>
                    {saving ? <RefreshCw size={14} className="spin" /> : <Check size={14} color="var(--success-color)" />}
                  </button>
                  <button className="btn-icon" onClick={() => setEditingPath(null)} disabled={saving}>
                    <X size={14} color="var(--danger-color)" />
                  </button>
                </div>
              )}
            </>
          )}
        </div>
        {node.expanded && node.children && (
          <div className="tree-node">
            {renderTree(node.children)}
          </div>
        )}
      </div>
    ));
  };

  return (
    <div className="tree-container">
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '1rem' }}>
        <h3 style={{ fontSize: '1.1rem', fontWeight: '500' }}>Datamodel Browser</h3>
        <button className="btn-secondary" onClick={() => fetchLevel('')} style={{ padding: '0.4rem 0.8rem', fontSize: '0.875rem' }}>
          <RefreshCw size={14} className={loading ? 'spin' : ''} /> Refresh Root
        </button>
      </div>

      {error && (
        <div style={{ color: 'var(--danger-color)', marginBottom: '1rem', fontSize: '0.875rem' }}>
          {error}
        </div>
      )}

      <div style={{ background: 'rgba(0,0,0,0.2)', padding: '1rem', borderRadius: '8px', minHeight: '300px' }}>
        {Object.keys(tree).length === 0 && !loading ? (
          <p style={{ color: 'var(--text-secondary)' }}>No objects found.</p>
        ) : (
          renderTree(tree)
        )}
      </div>
    </div>
  );
}
