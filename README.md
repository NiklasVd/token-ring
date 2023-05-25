# Token Ring
## Aufgaben zum Rechnernetze-Modul

Der Ordner token-ring beinhaltet die Bibliothek für die Netzwerkkapabilität und die Pakete/Tokens.
Zum Testen des Token Rings einfach token-ring-chat-auth starten (d.h. die *Active Station*) und die Nodes (*Passive Stations*) lassen sich
mit token-ring-chat starten.

## Digitale Signaturen
Die Funktionalität für digitale Signaturen ist in **signature.rs** definiert.
Eine digitale Signatur wird folgendermaßen erstellt:
  1. Station generiert Schlüsselpaar (privat/öffentlich).
  2. Neues Paket mit Header (ID, Zeitstempel) wird erstellt.
  3. Zum Header wird digitale Signatur erstellt und neben Header-Bytes vor Paketsegment platziert.
  4. Öffentlicher Schlüssel wird zusammen mit Paketen verschickt.
  5. Andere Stationen können mittels öffentlichem Schlüssel Authentizität/Integrität der Daten überprüfen.

Ein signierter Datencontainer sieht so aus:
```
struct Signed<T> {
  key: PublicKey, // 32 bytes
  signature: Signature, // 64 bytes
  val: T,
  val_bytes: Vec<u8>
}
```

Jeder Container wird bei Erstellung sofort signiert und der private Schlüssel sofort aus dem lokalen Speicher entlassen, um Datenlecks oder unnötiges Halten der Schlüssel im Heap zu vermeiden. On-demand Signierung wäre deutlich einfacher gewesen aber ich war mir nicht sicher, ob es sinnvoll ist, dass jeder erstelle Container eine Instanz des Schlüsselpaars hält, bis der Container wirklich abgeschickt (ergo signiert) wird.

Die *Active Stations* signieren jeden Token der im Ring weitergegeben wird. So lässt sich Spam und Spoofing innerhalb des
Netzwerks verhindern und *Passive Stations* können sich nicht selbst zum "Chef" des Netzwerks befördern.

## Netzwerk

Das Paketprotokoll (**packet.rs**) besteht aus einfachem Header und einem Inhaltssegment.
```
struct Packet {
  header: Signed<PacketHeader>, // (ID, Timestamp) + Key
  content: PacketType // Join, JoinReply, Token, Leave
}
```

Tokens (**token.rs**) bestehen ebenfalls aus Header und Frame(s).

```
struct Token {
  header: TokenHeader, // (Sender ID, Timestamp)
  frames: Vec<TokenFrame> // Frame ID (Sender ID, Timestamp), Frame (Empty, Data, Ack Data)
}
```

Die Token Header besitzten (u.a. aus Speichergründen) keine Signaturen, da die gesendeten Pakete der *Passive Stations* bereits signiert und zur Authentifizierung benutzt werden kann.

In der Datei **comm.rs** befinden sich die Sende- und Empfangsschleifen für die *Stations*. In **station.rs** ist die "Switch"-Logik der *Active Stations* und der Tokenumgang der *Passive Stations*. 
