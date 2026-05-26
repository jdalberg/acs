# acs
The project is built around the idea of an ACS server, which is used to manage devices.

## Diagrams

```mermaid
flowchart TD
    subgraph Devices["CPE Devices"]
        dev1(["CPE (CWMP)"])
        dev2(["CPE (USP)"])
        mock(["mock-client\n(CPE simulator)"])
    end

    subgraph Gateways["Protocol Gateways"]
        cwmp["acs-cwmp\nHTTP · TR-069"]
        usp["acs-usp\n(in progress)"]
    end

    subgraph Infra["Infrastructure"]
        nats{{NATS}}
        pg[(PostgreSQL)]
        redis[(Redis)]
    end

    subgraph Core["acs-controller"]
        ctrl["Event handler\n+ Provisioning engine\n+ HTTP API"]
        scripts["Provisioning scripts\n(Python)"]
        ctrl <-->|"discovers & runs"| scripts
    end

    subgraph Connreq["acs-connection-requester"]
        cr["Connection\nRequester"]
    end

    subgraph Frontend["acs-gui"]
        gui["React / Vite\nManagement UI"]
    end

    %% Device → Gateway
    dev1  -- "HTTP Inform" --> cwmp
    dev2  -- "USP Notify"  --> usp
    mock  -- "HTTP Inform" --> cwmp

    %% Gateway session store
    cwmp <--> |"session state"| redis

    %% Gateway → NATS (events)
    cwmp -- "acs.events.{oui}.{serial}.inform\nacs.events.{oui}.{serial}.command_response\nacs.events.{oui}.{serial}.session_ended" --> nats
    usp  -- "acs.events.…"  --> nats

    %% NATS → Controller
    nats -- "acs.events.>" --> ctrl

    %% Controller → DB
    ctrl <-->|"device state\nupsert / query"| pg

    %% Controller → NATS (commands)
    ctrl -- "acs.sessions.{session_id}.command" --> nats

    %% Controller → connection requester (via NATS)
    ctrl -- "acs.connection.request" --> nats
    nats -- "acs.connection.request" --> cr

    %% Connection requester → device
    cr -- "HTTP GET\n(wake-up)" --> dev1

    %% NATS → Gateway (commands)
    nats -- "acs.sessions.{session_id}.command" --> cwmp
    nats -- "acs.sessions.{session_id}.command" --> usp

    %% GUI → Controller API
    gui -- "REST /api/v1/…" --> ctrl
```

## Components

The project consists of the following components:

- **cwmp**: The CWMP component, which is responsible for managing CWMP devices. It implements the CWMP protocol.
- **usp**: The USP component, which is responsible for managing USP devices. It implements the USP protocol.
- **nats**: The NATS component, which is responsible for the communication between components of the ACS server.

## External influences

### Device connection
A device should be able to connect to this component, delivering it's event information, i.e. Inform with events in the CWMP case.

### Queue
Access to the queuing system where operations for this device is placed. If a queue has contents, handle at once and then signal the configuration controller and start a timer. This gives the core controller a chance to put new stuff into the queue which will be handled before the timeout when the session will be closed. The core controller should put an "end" message in the queue when the session should be closed, i.e. a message that says "no more operations are going to be sent in this session".

#### NATS subject design ####

##### Pod → Controller (Events)

- acs.events.{oui}.{serial}.inform
- acs.events.{oui}.{serial}.command_response
- acs.events.{oui}.{serial}.session_ended

Controller subscribes to acs.events.>. The device identity is baked into the subject, giving you per-device filtering and natural ordering with JetStream.

##### Controller → Pod (Commands)

- acs.sessions.{session_id}.command

Each pod subscribes only to its own session's subject after the Inform. The session_id is the routing key that pins commands to the right pod.

##### Full Flow

```mermaid
flowchart TD;
    subgraph CPE
        dev1((CPE))
    end
    
    subgraph Pod
        pod1[Pod]
    end
    
    subgraph Controller
        controller[Controller]
    end
    
    dev1 -- Inform --> pod1
    pod1 -- acs.events.{dev}.inform --> controller
    controller -- acs.sessions.{session_id}.command --> pod1
    pod1 -- Holds Command --> pod1
    pod1 -- POST --> dev1
    dev1 -- Response --> pod1
    pod1 -- acs.events.{dev}.command_response --> controller
```

Use JetStream for the event stream.
For the `acs.events.>` side, use a JetStream stream (e.g. ACS_EVENTS) so the controller gets:

- Guaranteed delivery even if it restarts
- Replay for debugging
- Exactly-once processing with consumers

The `acs.sessions.{session_id}.command` side can stay as plain core NATS (no persistence needed — the session is live, and if the pod dies the session dies too).



### Session initiation
The action of making a device initiate a new session is left to other components.