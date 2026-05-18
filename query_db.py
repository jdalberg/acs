import psycopg2

conn = psycopg2.connect("postgres://acs:acs@localhost/acs")
cur = conn.cursor()
cur.execute("SELECT * FROM device_protocols;")
rows = cur.fetchall()
print("Device Protocols:", rows)

cur.execute("SELECT * FROM devices;")
devices = cur.fetchall()
print("Devices:", devices)
