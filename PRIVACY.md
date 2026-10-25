# Privacy

VoidRecoverPS5 has no telemetry, analytics, accounts, crash reporting or uploads.

Network access happens only when you ask for it:

- **Refresh sources** reads three public pages: Sony's PS5 system software support page, the MewX firmware list on GitHub (raw.githubusercontent.com), and Darthsternie's archive (darthsternie.net). Requests carry a `User-Agent` naming the app and its version.
- **Check if Sony still serves it** sends one HEAD request to Sony's download server for that package. Nothing is downloaded.
- **Download from Sony** downloads one package from Sony's download server (pc.ps5.update.playstation.net) into the app's data folder. The address comes from the catalog; nothing is sent besides the request.
- **Open in browser** (catalog mirrors, community links, the "report" link) hands a link to your default browser after you confirm; the app itself doesn't contact that site.

Files you check are read on your computer and never sent anywhere. Folders you snapshot in the storage notebook are only listed (names, sizes, modified times); nothing is sent anywhere, and exports hide your user name and the folder's full path unless you turn that off.

Stored on your computer, in your Windows profile's app data folder for `com.voidfnc.voidrecoverps5`: the catalog cache (`catalog-cache.json`), the storage notebook (`notebook.json`), downloaded packages (`downloads`), and in the browser storage of the app window your language choice. Delete any of them at any time.
