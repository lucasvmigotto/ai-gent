"""Module docstring.

    Indented docstring line.
"""
import math


class Report:


    def __init__(self, rows):
        self.rows = rows  # trailing comment   

    def total(self):
        text = """
        triple-quoted

        with a blank line
        """
        return sum(r * 2 for r in self.rows), text


if __name__ == "__main__":
    r = Report([1, 2, 3])
    print(r.total()[0], math.pi > 3)
