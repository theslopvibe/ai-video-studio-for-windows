import type { Lang } from "./i18n";

// English first. Greek is the second language.
const W = {
  title: { en: "Welcome to AI Video Studio", el: "Καλώς ήρθες στο AI Video Studio" },
  lead: {
    en: "Turn a text prompt or a picture into a video or an image, using open-source AI models. This quick setup gets everything ready, step by step. It takes a few minutes, and most of that is downloading.",
    el: "Φτιάξε video ή εικόνα από ένα κείμενο ή μια φωτογραφία, με open-source μοντέλα AI. Αυτή η γρήγορη ρύθμιση ετοιμάζει τα πάντα, βήμα βήμα. Θέλει λίγα λεπτά και τον περισσότερο χρόνο τον πιάνει το κατέβασμα.",
  },
  free: { en: "Free and open source. Your prompts and videos stay on your computer.", el: "Δωρεάν και open source. Τα prompts και τα video σου μένουν στον υπολογιστή σου." },
  s1: { en: "Welcome", el: "Καλωσόρισμα" },
  s2: { en: "Where to run", el: "Πού θα τρέχει" },
  s3: { en: "Install", el: "Εγκατάσταση" },
  s4: { en: "Models", el: "Μοντέλα" },
  s5: { en: "First video", el: "Πρώτο video" },
  start: { en: "Get started", el: "Ξεκινάμε" },
  next: { en: "Continue", el: "Συνέχεια" },
  back: { en: "Back", el: "Πίσω" },
  skip: { en: "Skip setup, I'll do it later", el: "Παράλειψη, θα το κάνω αργότερα" },
  q_where: { en: "Where should videos be made?", el: "Πού θα φτιάχνονται τα video;" },
  scanning: { en: "Looking at your computer…", el: "Κοιτάζω τον υπολογιστή σου…" },
  found: { en: "Found in this PC", el: "Βρέθηκε σε αυτόν τον υπολογιστή" },
  no_gpu: { en: "No graphics card found that we can set up automatically.", el: "Δεν βρέθηκε κάρτα γραφικών που μπορούμε να στήσουμε αυτόματα." },
  exp_gpu: {
    en: "AMD and Intel cards work, but support is experimental and some models may be slower or fail. NVIDIA is the safest choice. The cloud always works.",
    el: "Οι κάρτες AMD και Intel δουλεύουν, αλλά η υποστήριξη είναι πειραματική και κάποια μοντέλα μπορεί να είναι πιο αργά ή να αποτυγχάνουν. Η NVIDIA είναι η ασφαλέστερη επιλογή. Το cloud δουλεύει πάντα.",
  },
  ram: { en: "RAM", el: "RAM" },
  disk: { en: "free disk", el: "ελεύθερος δίσκος" },
  pc: { en: "On this PC", el: "Σε αυτόν τον υπολογιστή" },
  pc_d: {
    en: "Free and private. Works best with an NVIDIA card. We install everything for you.",
    el: "Δωρεάν και ιδιωτικό. Δουλεύει καλύτερα με κάρτα NVIDIA. Τα εγκαθιστούμε όλα εμείς.",
  },
  cloud: { en: "Cloud GPU", el: "Cloud GPU" },
  cloud_d: {
    en: "Rent a powerful GPU by the minute (about 5 to 50 cents per video). No strong PC needed.",
    el: "Νοικιάζεις δυνατή GPU ανά λεπτό (περίπου 5 έως 50 λεπτά του δολαρίου ανά video). Δεν χρειάζεται δυνατός υπολογιστής.",
  },
  both: { en: "Both", el: "Και τα δύο" },
  both_d: { en: "Use this PC by default and switch to the cloud per video for heavy models.", el: "Προεπιλογή ο υπολογιστής σου, και cloud ανά video για τα βαριά μοντέλα." },
  recommended: { en: "Recommended", el: "Προτείνεται" },
  // install
  inst_pc: { en: "Install on this PC", el: "Εγκατάσταση σε αυτόν τον υπολογιστή" },
  inst_pc_d: {
    en: "We download ComfyUI (the engine that runs the AI models, about 2 GB) and set it up in the app folder. Nothing else on your PC is changed.",
    el: "Κατεβάζουμε το ComfyUI (η μηχανή που τρέχει τα μοντέλα AI, περίπου 2 GB) και το στήνουμε στον φάκελο της εφαρμογής. Τίποτα άλλο στον υπολογιστή σου δεν αλλάζει.",
  },
  install_btn: { en: "Install now", el: "Εγκατάσταση τώρα" },
  cancel: { en: "Cancel", el: "Ακύρωση" },
  have_comfy: { en: "I already have ComfyUI", el: "Έχω ήδη ComfyUI" },
  have_comfy_d: {
    en: "Paste the folder that contains “python_embeded” and “ComfyUI” (the portable Windows version).",
    el: "Επικόλλησε τον φάκελο που περιέχει τα «python_embeded» και «ComfyUI» (η portable έκδοση για Windows).",
  },
  use_folder: { en: "Use this folder", el: "Χρήση αυτού του φακέλου" },
  installed: { en: "ComfyUI is installed.", el: "Το ComfyUI είναι εγκατεστημένο." },
  testing: { en: "Starting the engine to check it works…", el: "Ξεκινάω τη μηχανή για να δω ότι δουλεύει…" },
  works: { en: "The engine started. This PC is ready.", el: "Η μηχανή ξεκίνησε. Ο υπολογιστής είναι έτοιμος." },
  p_tools: { en: "Downloading 7-Zip", el: "Κατεβάζω το 7-Zip" },
  p_comfy: { en: "Downloading ComfyUI", el: "Κατεβάζω το ComfyUI" },
  p_extract: { en: "Unpacking", el: "Αποσυμπίεση" },
  p_nodes: { en: "Installing extensions", el: "Εγκατάσταση επεκτάσεων" },
  p_models: { en: "Downloading models", el: "Κατεβάζω μοντέλα" },
  inst_cloud: { en: "Connect a cloud GPU provider", el: "Σύνδεση με πάροχο cloud GPU" },
  provider: { en: "Provider", el: "Πάροχος" },
  soon: { en: "coming soon", el: "σύντομα" },
  vast_how: {
    en: "1. Create a free account at vast.ai. 2. Add a few dollars of credit under Billing. 3. Open Account → API Keys, create a key and paste it here.",
    el: "1. Φτιάξε δωρεάν λογαριασμό στο vast.ai. 2. Βάλε λίγα δολάρια credit στο Billing. 3. Άνοιξε Account → API Keys, φτιάξε ένα κλειδί και επικόλλησέ το εδώ.",
  },
  fal_how: {
    en: "1. Create an account at fal.ai. 2. Add credit under Billing. 3. Open Dashboard → Keys, create a key and paste it here. fal.ai runs the models for you, so there is no GPU to rent. Each image or clip costs a few cents.",
    el: "1. Φτιάξε λογαριασμό στο fal.ai. 2. Βάλε credit στο Billing. 3. Άνοιξε Dashboard → Keys, φτιάξε ένα κλειδί και επικόλλησέ το εδώ. Το fal.ai τρέχει τα μοντέλα για σένα, άρα δεν νοικιάζεις GPU. Κάθε εικόνα ή clip κοστίζει λίγα cents.",
  },
  rep_how: {
    en: "1. Create an account at replicate.com. 2. Add credit under Billing. 3. Open Account → API tokens, create a token and paste it here. Replicate runs the models for you, so there is no GPU to rent. Each image or clip costs a few cents.",
    el: "1. Φτιάξε λογαριασμό στο replicate.com. 2. Βάλε credit στο Billing. 3. Άνοιξε Account → API tokens, φτιάξε ένα token και επικόλλησέ το εδώ. Το Replicate τρέχει τα μοντέλα για σένα, άρα δεν νοικιάζεις GPU. Κάθε εικόνα ή clip κοστίζει λίγα cents.",
  },
  api_key: { en: "API key", el: "API key" },
  save_test: { en: "Save and test", el: "Αποθήκευση και έλεγχος" },
  key_ok: { en: "Connected.", el: "Συνδέθηκε." },
  // models
  m_title: { en: "Choose your models", el: "Διάλεξε μοντέλα" },
  m_intro: {
    en: "Models are the AI brains that make the video. Bigger is better quality but slower and needs more graphics memory. We pre-selected one that fits your card.",
    el: "Τα μοντέλα είναι ο «εγκέφαλος» AI που φτιάχνει το video. Τα μεγαλύτερα βγάζουν καλύτερη ποιότητα αλλά είναι πιο αργά και θέλουν περισσότερη μνήμη κάρτας. Προεπιλέξαμε ένα που ταιριάζει στη δική σου.",
  },
  fits: { en: "fits your card", el: "ταιριάζει στην κάρτα σου" },
  tight: { en: "tight, may be slow", el: "οριακό, μπορεί να είναι αργό" },
  toobig: { en: "too big for your card", el: "πολύ μεγάλο για την κάρτα σου" },
  download_sel: { en: "Download selected", el: "Κατέβασμα επιλεγμένων" },
  m_cloud: {
    en: "With a cloud GPU the models live on the cloud worker. You manage them later in the Models page.",
    el: "Με cloud GPU τα μοντέλα βρίσκονται στον cloud worker. Τα διαχειρίζεσαι αργότερα από τη σελίδα Μοντέλα.",
  },
  have: { en: "downloaded", el: "κατεβασμένο" },
  // finish
  f_title: { en: "Make your first video", el: "Φτιάξε το πρώτο σου video" },
  f_intro: { en: "Press the button. You can edit the prompt first. Progress shows in the Queue.", el: "Πάτα το κουμπί. Μπορείς πρώτα να αλλάξεις το prompt. Η πρόοδος φαίνεται στην Ουρά." },
  f_sample: {
    en: "A small fox walking through fresh snow in a pine forest at sunrise, soft golden light, cinematic, slow camera follow",
    el: "A small fox walking through fresh snow in a pine forest at sunrise, soft golden light, cinematic, slow camera follow",
  },
  f_go: { en: "Generate my first video", el: "Φτιάξε το πρώτο μου video" },
  f_skip: { en: "Open the app without generating", el: "Άνοιγμα της εφαρμογής χωρίς παραγωγή" },
  f_prompt_note: { en: "Prompts work best in English.", el: "Τα prompts δουλεύουν καλύτερα στα αγγλικά." },
  need_nvidia_cloud: { en: "Choose Cloud GPU to continue.", el: "Διάλεξε Cloud GPU για να συνεχίσεις." },
} as const;

export type WKey = keyof typeof W;
export const wt = (lang: Lang, k: WKey): string => W[k][lang] ?? W[k].en;
