# External API surface of Assembly-CSharp (measured 2026-11-19)

Source: `tools/apiscan.sh` (metadata MemberRefs, not a grep). Raw detail in `re/api/*.members.txt` (local, not versioned).
`Assembly-CSharp-firstpass` = **Steamworks.NET only**, removed from the port (Steam stubbed).

423 external members in total; ~170 for UnityEngine, the rest = BCL (String, Math, DateTime, File, Linq, Convert...), Steam, Kartridge/Kongregate, Ookii (Windows dialogs).

## To implement in the engine (UnityEngine)
- **Base**: Object (Destroy, DontDestroyOnLoad, name, ==), GameObject (SetActive, AddComponent, GetComponent[InChildren/s], FindGameObjectsWithTag), Component, MonoBehaviour (Invoke, InvokeRepeating, CancelInvoke, StartCoroutine), Behaviour.enabled, Transform (position, localPosition, Find, GetChild, SetAsFirst/LastSibling, Translate, InverseTransformPoint), RectTransform.rect, Canvas.scaleFactor, CanvasRenderer.SetAlpha.
- **Maths**: Vector2/3, Quaternion.identity, Matrix4x4.TRS, Rect, Color/Color32 (+ operators, Lerp), Mathf (12 functions), Random (Range, value, InitState, state get/set: **the RNG must match Unity's** to reproduce results), Time.deltaTime.
- **UI**: Button.onClick, Text (text, fontSize, alignment), Image (sprite), RawImage.texture, Graphic.color, Selectable (interactable, image), Slider, Scrollbar, Toggle, InputField, Dropdown (8 members), UnityEvent/UnityAction, PointerEventData (button, clickCount, pointerId, pointerPress).
- **Input**: Input (mousePosition, GetMouseButton[Down/Up], GetKey[Down]) -> touch + Vita buttons.
- **System**: Application (dataPath, persistentDataPath, platform, Quit, OpenURL; ExternalCall/ExternalEval = WebGL -> stub), Screen/Display/Resolution, QualitySettings.vSyncCount, PlayerPrefs (Get/Set Int/String), Resources.Load, TextAsset.text, Debug.Log*.
- **Dynamic rendering**: Texture2D (ctor, SetPixel, Apply), Sprite.Create, ImageConversion.EncodeToPNG, AudioSource (Play, clip, mute).

## To remove / stub
- GUI/GUILayout (7 members): IMGUI debug screen -> remove.
- WWW/WWWForm: network (Kongregate/updates) -> offline stub.
- Steamworks (SteamUserStats, SteamRemoteStorage, SteamAPI), Kongregate.KartridgeBindings -> stub.
- Ookii VistaFileDialog / SFB (OpenFileDialog): save import/export through a file -> replace.
- System.Security.Cryptography (SymmetricAlgorithm, RandomNumberGenerator): save encryption/import-export -> reproduce faithfully (PC save compatibility).
- BinaryFormatter: save format -> analyse first (compatibility).

## Absent from the code (good news)
TextMeshPro, Animator, DOTween, BigInteger, reflection, threads, async.
