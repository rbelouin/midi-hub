GCC=gcc
EXEC=midi-hub

$(EXEC):
	$(GCC) -o $(EXEC) -lportmidi src/main.c

clean:
	rm -f $(EXEC)

run:
	./$(EXEC)
