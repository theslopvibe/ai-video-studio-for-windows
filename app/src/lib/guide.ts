export type Section = { id: string; icon: string; el: { t: string; steps: string[]; note?: string }; en: { t: string; steps: string[]; note?: string } };

export const GUIDE: Section[] = [
  {
    id: "start", icon: "1",
    el: { t: "Πρώτη φορά: ρύθμιση (μία φορά μόνο)", steps: [
      "Ρυθμίσεις → κουτί «RunPod»: επικόλλησε το API key (runpod.io → Settings → API Keys, με πρόσβαση Serverless ανάγνωση και εγγραφή) και το endpoint id (runpod.io → Serverless → το endpoint σου) και πάτα «Αποθήκευση και έλεγχος». Πρέπει να γράψει ότι έγινε αποδεκτό.",
      "Δεν χρειάζεται να ξεκινάς ή να σταματάς κάτι: το πρόγραμμα ξυπνά μόνο του το RunPod όταν στέλνεις video και το βάζει για ύπνο όταν η ουρά μείνει άδεια. Μπορείς και χειροκίνητα με «Ξύπνα το RunPod» / «Βάλε το RunPod για ύπνο».",
    ], note: "Η πρώτη παραγωγή μετά από ύπνο αργεί 1-3 λεπτά, γιατί ξυπνά το RunPod και ξεκινά ένας worker. Οι επόμενες είναι γρήγορες. Στο RunPod τα μοντέλα είναι ήδη στο network volume του endpoint." },
    en: { t: "First time: setup (once only)", steps: [
      "Settings → “RunPod” box: paste the API key (runpod.io → Settings → API Keys, with Serverless read and write access) and the endpoint id (runpod.io → Serverless → your endpoint) and press “Save and test”. It should say it was accepted.",
      "You never have to start or stop anything: the app wakes RunPod up when you send a video and puts it to sleep when the queue stays empty. You can also do it by hand with “Wake up RunPod” / “Put RunPod to sleep”.",
    ], note: "The first video after sleep takes 1-3 minutes while RunPod wakes up and a worker starts. Later ones are fast. On RunPod the models are already on the endpoint's network volume." },
  },
  {
    id: "vast", icon: "V",
    el: { t: "Vast.ai (ενοικιαζόμενη κάρτα): ρύθμιση και προβλήματα", steps: [
      "Ρυθμίσεις → κουτί «Ποιο cloud χρησιμοποιείς;» → διάλεξε Vast.ai.",
      "Ρυθμίσεις → επικόλλησε το Vast.ai API key (cloud.vast.ai → Account → API Keys) και πάτα «Αποθήκευση».",
      "GPU & Κόστος → διάλεξε κάρτα (π.χ. RTX 4090) και πάτα «Εφαρμογή».",
      "Μοντέλα → «Ξεκίνα κάρτα». Την πρώτη φορά κατεβαίνουν τα μοντέλα (περίπου 15-25 λεπτά). Όταν γράψει «Έτοιμη», φτιάξε video. Αν δεν ξεκινήσεις κάρτα, ξεκινά μόνη της με το πρώτο video.",
      "Η κάρτα χρεώνεται όσο υπάρχει, ακόμα κι όταν δεν φτιάχνει video. Όταν τελειώσεις πάτα «Διαγραφή κάρτας». Κλείνει και μόνη της μετά από λίγα λεπτά αδράνειας.",
      "Κολλάει στην εκκίνηση πάνω από 25 λεπτά: Μοντέλα → «Διαγραφή κάρτας» και ξανά «Ξεκίνα κάρτα» (θα πάρει άλλο μηχάνημα).",
    ], note: "Μπορείς να έχεις και δεύτερη κάρτα ταυτόχρονα (Μοντέλα → «Κάρτες ταυτόχρονα»). Αυτές οι οδηγίες ισχύουν μόνο όταν το cloud είναι το Vast." },
    en: { t: "Vast.ai (rented card): setup and problems", steps: [
      "Settings → “Which cloud do you use?” box → pick Vast.ai.",
      "Settings → paste your Vast.ai API key (cloud.vast.ai → Account → API Keys) and press “Save”.",
      "GPU & Cost → pick a card (e.g. RTX 4090) and press “Apply”.",
      "Models → “Start card”. The first time the models download (about 15-25 minutes). When it says “Ready”, make videos. If you do not start a card, one starts by itself with the first video.",
      "The card is billed while it exists, even when it is not making a video. When you are done press “Delete card”. It also closes by itself after a few idle minutes.",
      "Stuck starting for over 25 minutes: Models → “Delete card”, then “Start card” again (it will get another machine).",
    ], note: "You can run a second card at the same time (Models → “Cards at the same time”). These steps apply only when the cloud is Vast." },
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
      "Μερικοί τρόποι θέλουν ΔΥΟ εικόνες (αρχική και τελική, «Start → End frame»): το μοντέλο φτιάχνει την κίνηση από τη μία στην άλλη. Βάλε και τις δύο.",
      "Για πολλές σκηνές: γράψε prompt και εικόνα, πάτα «Πρόσθεσε σκηνή», και ξανά για την επόμενη. Στο τέλος «Προσθήκη στην ουρά». Κάθε σκηνή γίνεται ξεχωριστό video (s01, s02…).",
      "Δίπλα στον τρόπο παραγωγής βλέπεις την εκτίμηση κόστους ανά αποτέλεσμα.",
      "Πάτα «Generate».",
    ], note: "Τα video των σκηνών δεν ενώνονται αυτόματα σε ένα· τα ενώνεις με οποιοδήποτε πρόγραμμα μοντάζ. Επιλογές: Wan 2.2 14B = καλύτερη ποιότητα, χρειάζεται GPU 24 GB. Wan 2.2 5B = οικονομικό, GPU 16 GB. LTX-Video = πολύ γρήγορο και φθηνό, χαμηλότερη ποιότητα." },
    en: { t: "Make a video from an image", steps: [
      "Generate → “Image → Video” tab (or drag an image onto the box).",
      "Choose or drop your image. It becomes the first frame of the video.",
      "In the prompt describe the MOTION you want (e.g. “camera slowly pushes in, hair moves in the wind”), not the image again.",
      "Pick a mode. Each mode shows how many images it needs and its VRAM.",
      "Some modes need TWO images (start and end, “Start → End frame”): the model creates the motion from one to the other. Add both.",
      "For several scenes: write a prompt and add an image, press “Add scene”, and repeat for the next. At the end press “Add to queue”. Each scene becomes its own video (s01, s02…).",
      "Next to the mode you see the estimated cost per result.",
      "Press “Generate”.",
    ], note: "Scene videos are not joined into one automatically; join them in any video editor. Options: Wan 2.2 14B = best quality, needs a 24 GB GPU. Wan 2.2 5B = budget, 16 GB GPU. LTX-Video = very fast and cheap, lower quality." },
  },
  {
    id: "addmodel", icon: "4",
    el: { t: "Πρόσθεσε νέο μοντέλο από το HuggingFace", steps: [
      "Μοντέλα → «Πρόσθεσε μοντέλο».",
      "Γράψε όνομα (π.χ. «wan 2.2 image to video») ή επικόλλησε link από huggingface.co και πάτα «Αναζήτηση».",
      "Διάλεξε ένα αποτέλεσμα. Η εφαρμογή βρίσκει μόνη της τι είναι (video ή εικόνα, κείμενο→video ή εικόνα→video), πόση κάρτα χρειάζεται, ποιότητα και ταχύτητα (εκτίμηση), άδεια χρήσης, και προτείνει τα σωστά αρχεία. Έλεγξε τα τσεκαρισμένα αρχεία· αν το repo έχει πολλές παραλλαγές, χρησιμοποίησε το φίλτρο (π.χ. i2v, fp8).",
      "Πάτα «Πρόσθεσε στη λίστα». Για να τρέχει στον υπολογιστή σου, τσέκαρέ το και πάτα «Κατέβασμα στον υπολογιστή». Στο RunPod τα μοντέλα μπαίνουν στο network volume του endpoint από την κονσόλα του RunPod.",
      "Αν γράφει «Έτοιμο για χρήση με…», δεν χρειάζεται κάτι άλλο. Αλλιώς πρόσθεσε το workflow του: Μοντέλα → «Πρόσθεσε workflow (.json)» (από τη σελίδα του μοντέλου ή από το ComfyUI: Export → API). Η εφαρμογή βρίσκει μόνη της τις ρυθμίσεις του (μέγεθος, καρέ, fps, steps, seed, αρνητικό prompt) και τις δείχνει στη «Δημιουργία». Αν έχει κόμβο «Load Image», εμφανίζεται στο «Εικόνα → Video».",
    ], note: "Προσοχή: χρειάζεται repo με αρχεία .safetensors ή .gguf. Μοντέλα που ζητούν λογαριασμό στο HuggingFace, ή σε μορφή «diffusers», δεν δουλεύουν. Ψάξε «ComfyUI», «repackaged», «fp8» ή «GGUF». Αν δεν υπάρχει workflow, το μοντέλο κατεβαίνει αλλά δεν παράγει." },
    en: { t: "Add a new model from HuggingFace", steps: [
      "Models → “Add model”.",
      "Type a name (e.g. “wan 2.2 image to video”) or paste a huggingface.co link and press “Search”.",
      "Pick a result. The app works out by itself what it is (video or image, text→video or image→video), how much GPU it needs, quality and speed (estimates), license, and recommends the right files. Check the ticked files; if the repo has many variants use the filter (e.g. i2v, fp8).",
      "Press “Add to list”. To run it on your PC, tick it and press “Download to this PC”. On RunPod, models are added to the endpoint's network volume from the RunPod console.",
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
      "Στο RunPod πληρώνεις ανά δευτερόλεπτο, μόνο όσο η GPU φτιάχνει video (A40: περίπου 1,22 $ την ώρα). Όταν το RunPod κοιμάται δεν μπορεί να χρεώσει τίποτα.",
      "Πριν το Generate βλέπεις μια εκτίμηση κόστους ανά clip. Η πραγματική τιμή γράφεται στη Βιβλιοθήκη μετά.",
      "Για φθηνότερα video: μικρότερο μέγεθος, λιγότερα καρέ, μοντέλο «Γρήγορο (Lightning)» ή LTX-Video.",
    ] },
    en: { t: "Cost", steps: [
      "On RunPod you pay per second, only while the GPU is making a video (A40: about $1.22 per hour). While RunPod is asleep nothing can be billed.",
      "Before Generate you see an estimated cost per clip. The real cost is stored in Library afterwards.",
      "For cheaper videos: smaller size, fewer frames, a “Fast (Lightning)” mode or LTX-Video.",
    ] },
  },
  {
    id: "help", icon: "?",
    el: { t: "Κάτι δεν δουλεύει", steps: [
      "«Λείπει το API key»: Ρυθμίσεις → κουτί RunPod → επικόλλησε το κλειδί και το endpoint id.",
      "Κολλάει στο «Ξυπνάει το RunPod» πάνω από 2 λεπτά: Ρυθμίσεις → «Ξύπνα το RunPod», και ξαναδοκίμασε. Αν το κλειδί δεν έχει δικαίωμα εγγραφής στο Serverless, φτιάξε νέο.",
      "«Out of memory»: το μοντέλο δεν χωράει στη GPU. Διάλεξε μικρότερο μοντέλο ή μέγεθος.",
      "Το νέο μοντέλο δεν εμφανίζεται στη «Δημιουργία»: λείπει το workflow του (Μοντέλα → «Πρόσθεσε workflow»).",
    ] },
    en: { t: "Something doesn't work", steps: [
      "“API key missing”: Settings → RunPod box → paste the key and the endpoint id.",
      "Stuck on “Waking up RunPod” for over 2 minutes: Settings → “Wake up RunPod”, then try again. If your key has no Serverless write access, create a new one.",
      "“Out of memory”: the model doesn't fit the GPU. Pick a smaller model or size.",
      "New model doesn't show in Generate: its workflow is missing (Models → “Add workflow”).",
    ] },
  },
];
