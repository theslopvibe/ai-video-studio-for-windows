export type Section = { id: string; icon: string; el: { t: string; steps: string[]; note?: string }; en: { t: string; steps: string[]; note?: string } };

export const GUIDE: Section[] = [
  {
    id: "start", icon: "1",
    el: { t: "Πρώτη φορά: ρύθμιση (μία φορά μόνο)", steps: [
      "Ρυθμίσεις → επικόλλησε το Vast.ai API key (από cloud.vast.ai → Account → API Keys) και πάτα «Test Connection». Πρέπει να γράψει «Σύνδεση OK».",
      "Ρυθμίσεις → πάτα «Εφάρμοσε scale-to-zero», για να μην πληρώνεις όταν δεν φτιάχνεις video.",
      "GPU & Κόστος → διάλεξε μία ή δύο κάρτες (η φθηνότερη που χωράει το μοντέλο σου) και πάτα «Εφαρμογή».",
      "Μοντέλα → πάτα «Εγκατάσταση στους workers». Χρειάζεται μία φορά, και ξανά κάθε φορά που προσθέτεις μοντέλο.",
    ], note: "Η πρώτη παραγωγή μετά από αδράνεια αργεί 1-3 λεπτά, γιατί ξεκινά η cloud κάρτα. Οι επόμενες είναι γρήγορες." },
    en: { t: "First time: setup (once only)", steps: [
      "Settings → paste your Vast.ai API key (cloud.vast.ai → Account → API Keys) and press “Test Connection”. It should say OK.",
      "Settings → press “Apply scale-to-zero” so you don't pay while idle.",
      "GPU & Cost → pick one or two cards (the cheapest that fits your model) and press “Apply”.",
      "Models → press “Install on workers”. Needed once, and again every time you add a model.",
    ], note: "The first video after idle takes 1-3 minutes while the cloud GPU starts. Later ones are fast." },
  },
  {
    id: "t2v", icon: "2",
    el: { t: "Φτιάξε video από κείμενο", steps: [
      "Δημιουργία → καρτέλα «Κείμενο → Video».",
      "Γράψε τι θες να δεις (ελληνικά ή αγγλικά): θέμα, κίνηση, χώρος, φως, κάμερα.",
      "Αν θες καλύτερο prompt, πάτα ✨ «Βελτίωσε το prompt» (έχει και «Αναίρεση»).",
      "Διάλεξε μέγεθος (16:9, 9:16…) και πάτα «Generate». Η πρόοδος φαίνεται στην Ουρά.",
      "Όταν τελειώσει, το video είναι στη Βιβλιοθήκη και στον φάκελο του project.",
    ], note: "Πολλά prompts μαζί: τσέκαρε «Πολλά prompts» και γράψε ένα ανά γραμμή." },
    en: { t: "Make a video from text", steps: [
      "Generate → “Text → Video” tab.",
      "Describe what you want to see: subject, motion, place, light, camera.",
      "For a better prompt press ✨ “Improve prompt” (with Undo).",
      "Pick a size (16:9, 9:16…) and press “Generate”. Progress shows in Queue.",
      "When done, the video is in Library and in the project folder.",
    ], note: "Many prompts at once: tick “Multiple prompts” and write one per line." },
  },
  {
    id: "i2v", icon: "3",
    el: { t: "Φτιάξε video από εικόνα", steps: [
      "Δημιουργία → καρτέλα «Εικόνα → Video» (ή σύρε μια εικόνα πάνω στο πλαίσιο).",
      "Διάλεξε ή σύρε την εικόνα σου. Γίνεται το πρώτο καρέ του video.",
      "Στο prompt γράψε ΤΗΝ ΚΙΝΗΣΗ που θες (π.χ. «η κάμερα πλησιάζει αργά, τα μαλλιά κινούνται στον αέρα»), όχι να περιγράψεις ξανά την εικόνα.",
      "Διάλεξε τρόπο παραγωγής. Κάθε τρόπος γράφει πόσες εικόνες θέλει και πόση VRAM.",
      "Αν γράφει κόκκινο «το μοντέλο δεν έχει εγκατασταθεί», πάτα «Άνοιξε τα Μοντέλα» και μετά «Εγκατάσταση στους workers».",
      "Μερικοί τρόποι θέλουν ΔΥΟ εικόνες (αρχική και τελική, «Start → End frame»): το μοντέλο φτιάχνει την κίνηση από τη μία στην άλλη. Βάλε και τις δύο.",
      "Για πολλές σκηνές: γράψε prompt και εικόνα, πάτα «Πρόσθεσε σκηνή», και ξανά για την επόμενη. Στο τέλος «Προσθήκη στην ουρά». Κάθε σκηνή γίνεται ξεχωριστό video (s01, s02…).",
      "Μπορείς να διαλέξεις και ποια κάρτα γραφικών θα τρέξει (με την τιμή της ανά ώρα και την εκτίμηση κόστους ανά clip).",
      "Πάτα «Generate».",
    ], note: "Τα video των σκηνών δεν ενώνονται αυτόματα σε ένα· τα ενώνεις με οποιοδήποτε πρόγραμμα μοντάζ. Επιλογές: Wan 2.2 14B = καλύτερη ποιότητα, χρειάζεται GPU 24 GB. Wan 2.2 5B = οικονομικό, GPU 16 GB. LTX-Video = πολύ γρήγορο και φθηνό, χαμηλότερη ποιότητα." },
    en: { t: "Make a video from an image", steps: [
      "Generate → “Image → Video” tab (or drag an image onto the box).",
      "Choose or drop your image. It becomes the first frame of the video.",
      "In the prompt describe the MOTION you want (e.g. “camera slowly pushes in, hair moves in the wind”), not the image again.",
      "Pick a mode. Each mode shows how many images it needs and its VRAM.",
      "If you see the red “model not installed” notice, press “Open Models” and then “Install on workers”.",
      "Some modes need TWO images (start and end, “Start → End frame”): the model creates the motion from one to the other. Add both.",
      "For several scenes: write a prompt and add an image, press “Add scene”, and repeat for the next. At the end press “Add to queue”. Each scene becomes its own video (s01, s02…).",
      "You can also pick which graphics card runs it (with its hourly price and the estimated cost per clip).",
      "Press “Generate”.",
    ], note: "Scene videos are not joined into one automatically; join them in any video editor. Options: Wan 2.2 14B = best quality, needs a 24 GB GPU. Wan 2.2 5B = budget, 16 GB GPU. LTX-Video = very fast and cheap, lower quality." },
  },
  {
    id: "addmodel", icon: "4",
    el: { t: "Πρόσθεσε νέο μοντέλο από το HuggingFace", steps: [
      "Μοντέλα → «Πρόσθεσε μοντέλο».",
      "Γράψε όνομα (π.χ. «wan 2.2 image to video») ή επικόλλησε link από huggingface.co και πάτα «Αναζήτηση».",
      "Διάλεξε ένα αποτέλεσμα. Η εφαρμογή βρίσκει μόνη της τι είναι (video ή εικόνα, κείμενο→video ή εικόνα→video), πόση κάρτα χρειάζεται, ποιότητα και ταχύτητα (εκτίμηση), άδεια χρήσης, και προτείνει τα σωστά αρχεία. Έλεγξε τα τσεκαρισμένα αρχεία· αν το repo έχει πολλές παραλλαγές, χρησιμοποίησε το φίλτρο (π.χ. i2v, fp8).",
      "Πάτα «Πρόσθεσε στη λίστα», μετά τσέκαρε το μοντέλο στη λίστα και πάτα «Εγκατάσταση στους workers».",
      "Αν γράφει «Έτοιμο για χρήση με…», δεν χρειάζεται κάτι άλλο. Αλλιώς πρόσθεσε το workflow του: Μοντέλα → «Πρόσθεσε workflow (.json)» (από τη σελίδα του μοντέλου ή από το ComfyUI: Export → API). Η εφαρμογή βρίσκει μόνη της τις ρυθμίσεις του (μέγεθος, καρέ, fps, steps, seed, αρνητικό prompt) και τις δείχνει στη «Δημιουργία». Αν έχει κόμβο «Load Image», εμφανίζεται στο «Εικόνα → Video».",
    ], note: "Προσοχή: χρειάζεται repo με αρχεία .safetensors ή .gguf. Μοντέλα που ζητούν λογαριασμό στο HuggingFace, ή σε μορφή «diffusers», δεν δουλεύουν. Ψάξε «ComfyUI», «repackaged», «fp8» ή «GGUF». Αν δεν υπάρχει workflow, το μοντέλο κατεβαίνει αλλά δεν παράγει." },
    en: { t: "Add a new model from HuggingFace", steps: [
      "Models → “Add model”.",
      "Type a name (e.g. “wan 2.2 image to video”) or paste a huggingface.co link and press “Search”.",
      "Pick a result. The app works out by itself what it is (video or image, text→video or image→video), how much GPU it needs, quality and speed (estimates), license, and recommends the right files. Check the ticked files; if the repo has many variants use the filter (e.g. i2v, fp8).",
      "Press “Add to list”, then tick the model in the list and press “Install on workers”.",
      "If it says “Ready to use with…” nothing else is needed. Otherwise add its workflow: Models → “Add workflow (.json)” (from the model's page or ComfyUI: Export → API). The app detects its settings (size, frames, fps, steps, seed, negative prompt) and shows them on Generate. If it has a “Load Image” node it shows up in “Image → Video”.",
    ], note: "Note: you need a repo with .safetensors or .gguf files. Models that need a HuggingFace account, or in “diffusers” format, don't work. Look for “ComfyUI”, “repackaged”, “fp8” or “GGUF”. Without a workflow the model downloads but can't generate." },
  },
  {
    id: "files", icon: "5",
    el: { t: "Πού βρίσκονται τα αρχεία μου", steps: [
      "Τα video σώζονται στον φάκελο του project (Projects → επιλέγεις φάκελο). Αλλιώς στα Βίντεο\\AI Video Studio.",
      "Βιβλιοθήκη: δες όλα τα video, παίξε τα, άνοιξε τον φάκελο, ξαναχρησιμοποίησε το prompt ή διέγραψε.",
      "Η εικόνα εκκίνησης κρατιέται στο %APPDATA%\\com.aivideostudio.app\\inputs.",
    ] },
    en: { t: "Where are my files", steps: [
      "Videos are saved in the project folder (Projects → choose a folder). Otherwise in Videos\\AI Video Studio.",
      "Library: see all videos, play them, open the folder, reuse the prompt or delete.",
      "Start images are kept in %APPDATA%\\com.aivideostudio.app\\inputs.",
    ] },
  },
  {
    id: "cost", icon: "6",
    el: { t: "Κόστος", steps: [
      "Πληρώνεις μόνο όσο δουλεύει η cloud κάρτα (ανά ώρα, στο Vast.ai). Όταν δεν παράγεις, δεν πληρώνεις (scale-to-zero).",
      "Πριν το Generate βλέπεις μια εκτίμηση κόστους ανά clip. Η πραγματική τιμή γράφεται στη Βιβλιοθήκη μετά.",
      "Για φθηνότερα video: μικρότερο μέγεθος, λιγότερα καρέ, μοντέλο «Γρήγορο (Lightning)» ή LTX-Video.",
    ] },
    en: { t: "Cost", steps: [
      "You pay only while the cloud GPU works (hourly, at Vast.ai). When idle you pay nothing (scale-to-zero).",
      "Before Generate you see an estimated cost per clip. The real cost is stored in Library afterwards.",
      "For cheaper videos: smaller size, fewer frames, a “Fast (Lightning)” mode or LTX-Video.",
    ] },
  },
  {
    id: "help", icon: "?",
    el: { t: "Κάτι δεν δουλεύει", steps: [
      "«Λείπει το API key»: Ρυθμίσεις → επικόλλησε το κλειδί του Vast.",
      "Κολλάει στο «Εκκίνηση worker» πάνω από 5-10 λεπτά: δεν βρίσκεται διαθέσιμη κάρτα. Πρόσθεσε κι άλλη κάρτα στο «GPU & Κόστος».",
      "«Out of memory»: το μοντέλο δεν χωράει στην κάρτα. Διάλεξε μεγαλύτερη κάρτα ή μικρότερο μοντέλο/μέγεθος.",
      "Το video δεν σώζεται: Μοντέλα → «Εγκατάσταση στους workers» (ενεργοποιεί την τοπική αποθήκευση).",
      "Το νέο μοντέλο δεν εμφανίζεται στη «Δημιουργία»: λείπει το workflow ή δεν έγινε «Εγκατάσταση στους workers».",
      "Μετά από «Εγκατάσταση στους workers» ισχύει για ΝΕΟΥΣ workers. Ένας ζεστός worker αλλάζει όταν κατέβει και ξαναξεκινήσει.",
    ] },
    en: { t: "Something doesn't work", steps: [
      "“API key missing”: Settings → paste your Vast key.",
      "Stuck on “Starting worker” over 5-10 minutes: no GPU is available. Add another card in “GPU & Cost”.",
      "“Out of memory”: the model doesn't fit the card. Pick a bigger card or a smaller model/size.",
      "Video isn't saved: Models → “Install on workers” (enables local saving).",
      "New model doesn't show in Generate: its workflow is missing or “Install on workers” wasn't done.",
      "After “Install on workers” it applies to NEW workers. A warm worker updates once it scales down and restarts.",
    ] },
  },
];
