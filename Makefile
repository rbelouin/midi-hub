GCC=gcc
EXEC=midi-hub

$(EXEC):
	$(GCC) -o $(EXEC) src/main.c

clean:
	rm -f $(EXEC)
