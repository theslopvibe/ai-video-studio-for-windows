# AI Video Studio For Windows για Claude Desktop

Το Claude μιλά με την εφαρμογή AI Video Studio For Windows και φτιάχνει video από το chat.

## Εγκατάσταση (μία φορά)
1. Εγκατάστησε/ενημέρωσε το **AI Video Studio For Windows** (`AI Video Studio For Windows_0.1.0_x64-setup.exe`, η έκδοση με το «τοπικό API»).
2. Άνοιξε την εφαρμογή, βάλε το RunPod API key και το endpoint id (Settings), όπως στις Οδηγίες της.
3. Διπλό κλικ στο `ai-video-studio.mcpb`, μετά «Install» στο Claude Desktop (ή Settings → Extensions → Advanced → Install Extension).
4. Άνοιξε νέο chat. Στα εργαλεία πρέπει να φαίνεται το «AI Video Studio For Windows».

## Χρήση
Η εφαρμογή πρέπει να είναι ανοιχτή. Στο chat γράφεις π.χ.:
«Φτιάξε 5 πλάνα για ιστορία με ένα αλεπουδάκι στο χιόνι, 4 δευτερόλεπτα το καθένα, με LTX.»
Το Claude θα σου πει το κόστος και θα ρωτήσει πριν ξεκινήσει. Μετά δίνει τα mp4 στα skills μοντάζ που έχεις.

## Ασφάλεια
Το API ακούει μόνο στο `127.0.0.1` και θέλει token (αρχείο `%APPDATA%\com.aivideostudio.app\api.json`). Κάθε παραγωγή θέλει ρητή επιβεβαίωση από εσένα.
