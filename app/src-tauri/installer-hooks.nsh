; Uninstall hooks for AI Video Studio (Tauri NSIS). English first, Greek when the installer language is Greek.
; Asks before deleting anything. A silent uninstall (/S) never deletes user data.

Var AIVS_DelData
Var AIVS_DelMedia
Var AIVS_Q1
Var AIVS_Q2
Var AIVS_C1
Var AIVS_C2

!macro NSIS_HOOK_PREUNINSTALL
  StrCpy $AIVS_DelData "0"
  StrCpy $AIVS_DelMedia "0"
  StrCpy $AIVS_Q1 "Also delete the data the app stored on this computer?$\r$\n$\r$\n- the AI models and the local ComfyUI engine you downloaded (can be many GB)$\r$\n- your settings, queue, chat history and saved API keys$\r$\n$\r$\nChoose No to keep everything (you can reinstall later and continue where you stopped)."
  StrCpy $AIVS_Q2 "Also delete the videos and images you created?$\r$\n$\r$\nFolder: $VIDEOS\AI Video Studio$\r$\n(Projects saved in other folders are never touched.)"
  StrCpy $AIVS_C1 "Last check. This will permanently delete the AI models, ComfyUI, settings, chat history and API keys.$\r$\n$\r$\nThis cannot be undone. Delete them?"
  StrCpy $AIVS_C2 "Last check. This will permanently delete the AI models, ComfyUI, settings, chat history, API keys AND all videos and images you created in $VIDEOS\AI Video Studio.$\r$\n$\r$\nThis cannot be undone. Delete everything?"
  ${If} $LANGUAGE == 1032
    StrCpy $AIVS_Q1 "Να διαγράψω και τα δεδομένα που αποθήκευσε η εφαρμογή σε αυτόν τον υπολογιστή;$\r$\n$\r$\n- τα μοντέλα AI και η τοπική μηχανή ComfyUI που κατέβασες (μπορεί να είναι πολλά GB)$\r$\n- οι ρυθμίσεις, η ουρά, το ιστορικό chat και τα αποθηκευμένα API keys$\r$\n$\r$\nΕπίλεξε Όχι για να κρατήσεις τα πάντα (μπορείς να ξαναεγκαταστήσεις αργότερα και να συνεχίσεις από εκεί που σταμάτησες)."
    StrCpy $AIVS_Q2 "Να διαγράψω και τα video και τις εικόνες που έφτιαξες;$\r$\n$\r$\nΦάκελος: $VIDEOS\AI Video Studio$\r$\n(Τα projects που είναι σε άλλους φακέλους δεν αγγίζονται ποτέ.)"
    StrCpy $AIVS_C1 "Τελευταίος έλεγχος. Θα διαγραφούν οριστικά τα μοντέλα AI, το ComfyUI, οι ρυθμίσεις, το ιστορικό chat και τα API keys.$\r$\n$\r$\nΔεν γίνεται αναίρεση. Να διαγραφούν;"
    StrCpy $AIVS_C2 "Τελευταίος έλεγχος. Θα διαγραφούν οριστικά τα μοντέλα AI, το ComfyUI, οι ρυθμίσεις, το ιστορικό chat, τα API keys ΚΑΙ όλα τα video και οι εικόνες που έφτιαξες στο $VIDEOS\AI Video Studio.$\r$\n$\r$\nΔεν γίνεται αναίρεση. Να διαγραφούν όλα;"
  ${EndIf}
  IfSilent aivs_prompts_done
  MessageBox MB_YESNO|MB_ICONQUESTION|MB_DEFBUTTON2 $AIVS_Q1 IDNO aivs_prompts_done
  MessageBox MB_YESNO|MB_ICONQUESTION|MB_DEFBUTTON2 $AIVS_Q2 IDNO aivs_media_no
  StrCpy $AIVS_DelMedia "1"
  MessageBox MB_YESNO|MB_ICONEXCLAMATION|MB_DEFBUTTON2 $AIVS_C2 IDYES aivs_confirmed
  Goto aivs_declined
  aivs_media_no:
  MessageBox MB_YESNO|MB_ICONEXCLAMATION|MB_DEFBUTTON2 $AIVS_C1 IDYES aivs_confirmed
  aivs_declined:
  StrCpy $AIVS_DelMedia "0"
  Goto aivs_prompts_done
  aivs_confirmed:
  StrCpy $AIVS_DelData "1"
  aivs_prompts_done:
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $AIVS_DelData == "1"
    ; models, ComfyUI, database, logs, inputs (all inside the app data folder) and the WebView cache
    RMDir /r "$APPDATA\com.aivideostudio.app"
    RMDir /r "$LOCALAPPDATA\com.aivideostudio.app"
    ; API keys kept in the Windows Credential Manager
    nsExec::Exec 'cmdkey /delete:vast_api_key.AIVideoStudio'
    nsExec::Exec 'cmdkey /delete:anthropic_api_key.AIVideoStudio'
    nsExec::Exec 'cmdkey /delete:gemini_api_key.AIVideoStudio'
    nsExec::Exec 'cmdkey /delete:openai_api_key.AIVideoStudio'
    nsExec::Exec 'cmdkey /delete:glm_api_key.AIVideoStudio'
    nsExec::Exec 'cmdkey /delete:deepseek_api_key.AIVideoStudio'
    nsExec::Exec 'cmdkey /delete:openrouter_api_key.AIVideoStudio'
    nsExec::Exec 'cmdkey /delete:fal_key.AIVideoStudio'
    nsExec::Exec 'cmdkey /delete:replicate_token.AIVideoStudio'
    Pop $0
  ${EndIf}
  ${If} $AIVS_DelMedia == "1"
    RMDir /r "$VIDEOS\AI Video Studio"
  ${EndIf}
!macroend
