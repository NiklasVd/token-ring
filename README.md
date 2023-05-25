# nez
## Aufgaben zum Rechnernetze-Modul

Der Ordner token-ring beinhaltet die Bibliothek für die Netzwerkkapabilität und die Pakete/Tokens.
Die Funktionalität für digitale Signaturen ist in **signature.rs** definiert.

Zum Testen des Token Rings einfach token-ring-chat-auth starten (d.h. die *Active Station*) und die Nodes (*Passive Stations*) lassen sich
mit token-ring-chat starten.

## Digitale Signaturen
Eine digitale Signatur wird folgendermaßen erstellt:
  1. Station generiert Schlüsselpaar (privat/öffentlich).
  2. Neues Paket mit Header (ID, Zeitstempel) wird erstellt.
  3. Zum Header wird digitale Signatur erstellt und neben Header-Bytes vor Paketsegment platziert.
  4. Öffentlicher Schlüssel wird zusammen mit Paketen verschickt.
  5. Andere Stationen können mittels öffentlichem Schlüssel Authentizität/Integrität der Daten überprüfen.

Die *Active Stations* signieren jeden Token der im Ring weitergegeben wird. So lässt sich Spam und Spoofing innerhalb des
Netzwerks verhindern und *Passive Stations* können sich nicht selbst zum "Administrator" des Netzwerks befördern.
